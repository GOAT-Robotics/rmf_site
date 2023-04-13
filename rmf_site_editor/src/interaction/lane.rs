/*
 * Copyright (C) 2022 Open Source Robotics Foundation
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 *
*/

use crate::{interaction::*, site::*};
use bevy::prelude::*;

pub fn add_lane_visual_cues(
    mut commands: Commands,
    new_lane_segments: Query<(Entity, &LaneSkeleton), Added<LaneSkeleton>>,
) {
    for (e, segments) in &new_lane_segments {
        commands.entity(e).insert(VisualCue::no_outline());
        commands.entity(segments.mid).insert(Selectable::new(e));
    }
}

pub fn update_lane_visual_cues(
    mut lanes: Query<
        (&Hovered, &Selected, &LaneSkeleton, &mut Transform),
        (
            With<LaneMarker>,
            Without<AnchorVisualization>,
            Or<(Changed<Hovered>, Changed<Selected>, Changed<Edge<Entity>>)>,
        ),
    >,
    mut materials: Query<&mut Handle<StandardMaterial>>,
    mut visibility: Query<&mut Visibility>,
    site_assets: Res<SiteAssets>,
    cursor: Res<Cursor>,
) {
    for (hovering, selected, pieces, mut tf) in &mut lanes {
        if hovering.is_hovered {
            set_visibility(cursor.frame, &mut visibility, false);
        }

        let (m, h, v) = if hovering.cue() && selected.cue() {
            (
                &site_assets.hover_select_material,
                HOVERED_LANE_OFFSET,
                true,
            )
        } else if hovering.cue() {
            (&site_assets.hover_material, HOVERED_LANE_OFFSET, true)
        } else if selected.cue() {
            (&site_assets.select_material, SELECTED_LANE_OFFSET, true)
        } else {
            (&site_assets.unassigned_lane_material, 0.0, false)
        };

        for e in pieces.outlines {
            set_material(e, m, &mut materials);
            set_visibility(e, &mut visibility, v);
        }

        tf.translation.z = h;
    }
}
