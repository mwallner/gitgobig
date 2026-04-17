use gitgobig_core::CommitEntry;

/// Per-row graph drawing data.
#[derive(Clone, Debug)]
pub(crate) struct GraphRow {
    /// Column where the commit dot sits.
    pub(crate) commit_col: usize,
    /// Color index for the commit dot.
    pub(crate) commit_color: usize,
    /// Edges entering from the top: (from_col, to_col, color_index).
    pub(crate) incoming: Vec<(usize, usize, usize)>,
    /// Edges leaving from the bottom: (from_col, to_col, color_index).
    pub(crate) outgoing: Vec<(usize, usize, usize)>,
    /// Total number of columns active at this row (for width calculation).
    pub(crate) num_cols: usize,
}

/// Compute graph layout from a list of commits (topological order, newest first).
pub(crate) fn compute_graph(commits: &[CommitEntry]) -> Vec<GraphRow> {
    let mut lanes: Vec<Option<String>> = Vec::new();
    let mut lane_colors: Vec<usize> = Vec::new();
    let mut next_color: usize = 0;
    let mut rows = Vec::with_capacity(commits.len());

    for commit in commits {
        let expecting: Vec<usize> = lanes
            .iter()
            .enumerate()
            .filter_map(|(i, l)| {
                if l.as_deref() == Some(&commit.hash) {
                    Some(i)
                } else {
                    None
                }
            })
            .collect();

        let commit_col;
        let commit_color;

        if expecting.is_empty() {
            let col = lanes
                .iter()
                .position(|l| l.is_none())
                .unwrap_or(lanes.len());
            if col == lanes.len() {
                lanes.push(None);
                lane_colors.push(next_color);
                next_color += 1;
            }
            lanes[col] = Some(commit.hash.clone());
            lane_colors[col] = next_color;
            next_color += 1;
            commit_col = col;
            commit_color = lane_colors[col];
        } else {
            commit_col = expecting[0];
            commit_color = lane_colors[commit_col];
        }

        let lanes_before: Vec<Option<String>> = lanes.clone();
        let colors_before: Vec<usize> = lane_colors.clone();

        for &extra_lane in expecting.get(1..).unwrap_or_default() {
            lanes[extra_lane] = None;
        }

        if commit.parents.is_empty() {
            lanes[commit_col] = None;
        } else {
            lanes[commit_col] = Some(commit.parents[0].clone());

            for parent in &commit.parents[1..] {
                let already = lanes
                    .iter()
                    .position(|l| l.as_deref() == Some(parent.as_str()));
                if already.is_none() {
                    let col = lanes
                        .iter()
                        .position(|l| l.is_none())
                        .unwrap_or(lanes.len());
                    if col == lanes.len() {
                        lanes.push(None);
                        lane_colors.push(next_color);
                        next_color += 1;
                    } else {
                        lane_colors[col] = next_color;
                        next_color += 1;
                    }
                    lanes[col] = Some(parent.clone());
                }
            }
        }

        let mut incoming = Vec::new();
        for (i, prev_lane) in lanes_before.iter().enumerate() {
            if prev_lane.is_some() {
                if expecting.contains(&i) {
                    incoming.push((i, commit_col, colors_before[i]));
                } else {
                    incoming.push((i, i, colors_before[i]));
                }
            }
        }

        let mut outgoing = Vec::new();
        for (i, lane) in lanes.iter().enumerate() {
            if lane.is_some() {
                let from = if i < lanes_before.len()
                    && lanes_before[i].is_some()
                    && !expecting.get(1..).unwrap_or_default().contains(&i)
                {
                    i
                } else {
                    commit_col
                };
                outgoing.push((from, i, lane_colors[i]));
            }
        }

        let num_cols = lanes.len();

        rows.push(GraphRow {
            commit_col,
            commit_color,
            incoming,
            outgoing,
            num_cols,
        });
    }

    rows
}
