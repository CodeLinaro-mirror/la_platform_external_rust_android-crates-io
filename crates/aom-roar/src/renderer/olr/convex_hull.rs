// Copyright (c) 2026, Alliance for Open Media. All rights reserved
//
// This source code is subject to the terms of the BSD 3-Clause Clear License
// and the Alliance for Open Media Patent License 1.0. If the BSD 3-Clause Clear
// License was not distributed with this source code in the LICENSE file, you
// can obtain it at www.aomedia.org/license/software-license/bsd-3-c-c. If the
// Alliance for Open Media Patent License 1.0 was not distributed with this
// source code in the PATENTS file, you can obtain it at
// www.aomedia.org/license/patent.

#![forbid(unsafe_code)]

//! 2D convex hull solver implementation.
//!
//! This module implements a 2D convex hull solver using the Graham Scan algorithm.
//! It is used by the loudspeaker layout systems (like VBAP 2D) to find boundary
//! polygons for panning regions, matching the behavior of the C reference.

use crate::common::definitions::OarError;

/// Represents a speaker node in the layout with 2D coordinates and an associated index.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpeakerNode {
    /// The X-coordinate of the speaker node.
    pub x: f32,
    /// The Y-coordinate of the speaker node.
    pub y: f32,
    /// The original index of the speaker in the configuration or layout array.
    pub index: u32,
}

// TODO(b/525080422): Could be a speaker node method but could also potentially use built-in math.
fn dist_sq(p1: SpeakerNode, p2: SpeakerNode) -> f32 {
    (p1.x - p2.x) * (p1.x - p2.x) + (p1.y - p2.y) * (p1.y - p2.y)
}

// TODO(b/525080422): Replace magic values with types.
fn orientation(p: SpeakerNode, q: SpeakerNode, r: SpeakerNode) -> i32 {
    let val = (q.y - p.y) * (r.x - q.x) - (q.x - p.x) * (r.y - q.y);
    if val.abs() < 1e-9 {
        0
    } else if val > 0.0 {
        1 // Clockwise
    } else {
        2 // Counterclockwise
    }
}

/// Solves the 2D convex hull of a set of speaker nodes using the Graham Scan algorithm.
///
/// # Arguments
///
/// * `nodes` - A vector of speaker nodes to process.
///
/// # Errors
///
/// Returns `OarError::InvalidParameter` if the resulting hull has fewer than
/// three points (e.g., if there are fewer than three input points, or if they
/// are collinear).
pub fn solve_convex_hull(mut nodes: Vec<SpeakerNode>) -> Result<Vec<SpeakerNode>, OarError> {
    let n = nodes.len();
    if n < 3 {
        return Err(OarError::InvalidParameter);
    }

    // Find the bottom-most point (lowest Y)
    // If tie, find the left-most point (lowest X)
    let mut min_idx = 0;
    let mut ymin = nodes[0].y;
    for i in 1..n {
        let y = nodes[i].y;
        if (y < ymin) || ((y - ymin).abs() < 1e-9 && nodes[i].x < nodes[min_idx].x) {
            ymin = nodes[i].y;
            min_idx = i;
        }
    }

    // Swap the bottom-most point to the first position
    nodes.swap(0, min_idx);

    // Sort remaining points with respect to the first point.
    let p0 = nodes[0];
    nodes[1..].sort_by(|&p1, &p2| {
        let o = orientation(p0, p1, p2);
        if o == 0 {
            if dist_sq(p0, p2) >= dist_sq(p0, p1) {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            }
        } else if o == 2 {
            std::cmp::Ordering::Less
        } else {
            std::cmp::Ordering::Greater
        }
    });

    // If two or more points make same angle with p0, remove all but the one that is furthest from
    // p0.
    let mut m = 1;
    let mut i = 1;
    while i < n {
        while i < n - 1 && orientation(p0, nodes[i], nodes[i + 1]) == 0 {
            i += 1;
        }
        nodes[m] = nodes[i];
        m += 1;
        i += 1;
    }

    if m < 3 {
        return Err(OarError::InvalidParameter);
    }

    let mut stack = Vec::with_capacity(m);
    stack.push(nodes[0]);
    stack.push(nodes[1]);
    stack.push(nodes[2]);

    for (j, node) in nodes.iter().enumerate().take(m).skip(3) {
        while stack.len() > 1 {
            let top = stack[stack.len() - 1];
            let next_to_top = stack[stack.len() - 2];
            if orientation(next_to_top, top, *node) == 2 {
                break;
            }
            stack.pop();
        }
        stack.push(nodes[j]);
    }

    Ok(stack)
}

// ===== Tests =====

#[cfg(test)]
mod test {

    use super::*;
    use googletest::prelude::*;

    /// Helper to create a `SpeakerNode` with a default index.
    fn make_node(x: f32, y: f32, index: u32) -> SpeakerNode {
        SpeakerNode { x, y, index }
    }

    fn cross_product(p1: &SpeakerNode, p2: &SpeakerNode, p3: &SpeakerNode) -> f32 {
        (p2.x - p1.x) * (p3.y - p2.y) - (p2.y - p1.y) * (p3.x - p2.x)
    }

    #[gtest]
    fn test_solve_convex_hull_with_standard_layout_returns_extreme_corners_in_ccw_order() {
        let nodes = vec![
            make_node(0.0, 0.0, 0),   // internal
            make_node(1.0, 1.0, 1),   // corner
            make_node(0.5, 0.5, 2),   // internal
            make_node(-1.0, 1.0, 3),  // corner
            make_node(-1.0, -1.0, 4), // corner
            make_node(1.0, -1.0, 5),  // corner
            make_node(-0.2, 0.8, 6),  // internal
        ];

        let hull = solve_convex_hull(nodes).unwrap();

        expect_eq!(hull.len(), 4);

        expect_eq!(hull[0].x, -1.0);
        expect_eq!(hull[0].y, -1.0);

        expect_eq!(hull[1].x, 1.0);
        expect_eq!(hull[1].y, -1.0);

        expect_eq!(hull[2].x, 1.0);
        expect_eq!(hull[2].y, 1.0);

        expect_eq!(hull[3].x, -1.0);
        expect_eq!(hull[3].y, 1.0);
    }

    #[gtest]
    fn test_solve_convex_hull_with_nested_layout_filters_out_inner_points() {
        let nodes = vec![
            // Outer square
            make_node(-2.0, -2.0, 0),
            make_node(2.0, -2.0, 1),
            make_node(2.0, 2.0, 2),
            make_node(-2.0, 2.0, 3),
            // Inner square
            make_node(-1.0, -1.0, 4),
            make_node(1.0, -1.0, 5),
            make_node(1.0, 1.0, 6),
            make_node(-1.0, 1.0, 7),
            // More inner points
            make_node(0.0, 0.0, 8),
        ];

        let hull = solve_convex_hull(nodes).unwrap();

        expect_eq!(hull.len(), 4);

        expect_eq!(hull[0].x, -2.0);
        expect_eq!(hull[0].y, -2.0);

        expect_eq!(hull[1].x, 2.0);
        expect_eq!(hull[1].y, -2.0);

        expect_eq!(hull[2].x, 2.0);
        expect_eq!(hull[2].y, 2.0);

        expect_eq!(hull[3].x, -2.0);
        expect_eq!(hull[3].y, 2.0);
    }

    #[gtest]
    fn test_solve_convex_hull_with_degenerate_layouts_returns_invalid_parameter_error() {
        let hull_0 = solve_convex_hull(vec![]);
        expect_eq!(hull_0, Err(OarError::InvalidParameter));

        let p1 = make_node(1.2, 3.4, 0);
        let hull_1 = solve_convex_hull(vec![p1]);
        expect_eq!(hull_1, Err(OarError::InvalidParameter));

        let p2 = make_node(5.6, 7.8, 1);
        let hull_2 = solve_convex_hull(vec![p1, p2]);
        expect_eq!(hull_2, Err(OarError::InvalidParameter));

        let c1 = make_node(0.0, 0.0, 0);
        let c2 = make_node(1.0, 1.0, 1);
        let c3 = make_node(2.0, 2.0, 2);
        let hull_collinear = solve_convex_hull(vec![c1, c2, c3]);
        expect_eq!(hull_collinear, Err(OarError::InvalidParameter));
    }

    #[gtest]
    fn test_solve_convex_hull_filters_out_collinear_boundary_points() {
        let nodes = vec![
            make_node(0.0, 0.0, 0), // corner
            make_node(1.0, 0.0, 1), // boundary collinear
            make_node(2.0, 0.0, 2), // corner
            make_node(2.0, 1.0, 3), // boundary collinear
            make_node(2.0, 2.0, 4), // corner
            make_node(1.0, 2.0, 5), // boundary collinear
            make_node(0.0, 2.0, 6), // corner
            make_node(0.0, 1.0, 7), // boundary collinear
            make_node(1.0, 1.0, 8), // inner
        ];

        let hull = solve_convex_hull(nodes).unwrap();

        expect_eq!(hull.len(), 4);

        expect_eq!(hull[0].x, 0.0);
        expect_eq!(hull[0].y, 0.0);

        expect_eq!(hull[1].x, 2.0);
        expect_eq!(hull[1].y, 0.0);

        expect_eq!(hull[2].x, 2.0);
        expect_eq!(hull[2].y, 2.0);

        expect_eq!(hull[3].x, 0.0);
        expect_eq!(hull[3].y, 2.0);
    }

    #[gtest]
    fn test_solve_convex_hull_maintains_mathematical_convexity_and_inclusion() {
        let num_circle_points = 32;
        let mut nodes = Vec::new();
        let mut index = 0;

        for i in 0..num_circle_points {
            let angle = (i as f32) * 2.0 * std::f32::consts::PI / (num_circle_points as f32);
            let x = angle.cos();
            let y = angle.sin();
            nodes.push(make_node(x, y, index));
            index += 1;
        }

        for i in 0..50 {
            let angle = (i as f32) * 1.7;
            let radius = 0.8 * ((i as f32 * 3.1) % 1.0);
            let x = radius * angle.cos();
            let y = radius * angle.sin();
            nodes.push(make_node(x, y, index));
            index += 1;
        }

        let hull = solve_convex_hull(nodes.clone()).unwrap();

        let len = hull.len();
        expect_ge!(len, 3);

        for hull_node in &hull {
            let found = nodes.iter().any(|input_node| {
                input_node.index == hull_node.index
                    && (input_node.x - hull_node.x).abs() < 1e-6
                    && (input_node.y - hull_node.y).abs() < 1e-6
            });
            expect_true!(found);
        }

        for i in 0..len {
            let a = &hull[i];
            let b = &hull[(i + 1) % len];
            let c = &hull[(i + 2) % len];
            let cp = cross_product(a, b, c);
            expect_gt!(cp, 1e-5);
        }

        for input_node in &nodes {
            let mut inside = true;
            for i in 0..len {
                let a = &hull[i];
                let b = &hull[(i + 1) % len];
                let cp = cross_product(a, b, input_node);
                if cp < -1e-5 {
                    inside = false;
                    break;
                }
            }
            expect_true!(inside);
        }
    }
}
