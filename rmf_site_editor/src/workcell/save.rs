/*
 * Copyright (C) 2023 Open Source Robotics Foundation
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

use bevy::ecs::system::SystemState;
use bevy::prelude::*;
use std::path::PathBuf;
use std::collections::BTreeMap;

use crate::site::{DefaultFile, Pending};

use thiserror::Error as ThisError;

use rmf_site_format::*;

/// Event used to trigger saving of the workcell
pub struct SaveWorkcell {
    pub root: Entity,
    pub to_file: PathBuf,
}

#[derive(ThisError, Debug, Clone)]
pub enum WorkcellGenerationError {
    #[error("the specified entity [{0:?}] does not refer to a workcell")]
    InvalidWorkcellEntity(Entity),
}

// This is mostly duplicated with the function in site/save.rs, however this case
// is a lot simpler, also site/save.rs checks for children of levels but there are no levels here
fn assign_site_ids(world: &mut World, workcell: Entity) {
    // TODO(luca) actually keep site IDs instead of always generating them from scratch
    // (as it is done in site editor)
    let mut state: SystemState<(
        Query<Entity, (Or<(With<Anchor>, With<ModelMarker>)>, Without<Pending>)>,
        Query<&Children>,
    )> = SystemState::new(world);
    let (q_used_entities, q_children) = state.get(&world);

    let mut new_entities = vec!(workcell);
    for e in q_children.iter_descendants(workcell) {
        if let Ok(_) = q_used_entities.get(e) {
            new_entities.push(e);
        }
    }

    for (idx, entity) in new_entities.iter().enumerate() {
        world.entity_mut(*entity).insert(SiteID(idx.try_into().unwrap()));
    }
}

fn generate_anchors(
    q_anchors: &Query<(&Anchor, &SiteID, &Parent)>,
    q_ids: &Query<&SiteID>,
) -> BTreeMap<u32, Parented<u32, Frame>> {
    let mut anchors = BTreeMap::new();

    for (anchor, id, parent) in q_anchors {
        let parent = match q_ids.get(parent.get()) {
            Ok(parent) => Some(parent.0),
            Err(_) => None,
        };
        anchors.insert(
            id.0,
            Parented {
                parent: parent,
                bundle: Frame {
                    anchor: anchor.clone(),
                    marker: FrameMarker,
                }
            },
        );
    }

    anchors
}

pub fn generate_workcell(
    world: &mut World,
    root: Entity,
) -> Result<rmf_site_format::Workcell, WorkcellGenerationError> {
    assign_site_ids(world, root);
    let mut state: SystemState<(
        Query<(&Anchor, &SiteID, &Parent, Option<&MeshConstraint<Entity>>)>,
        Query<
            (
                &NameInSite,
                &AssetSource,
                &Pose,
                &IsStatic,
                &ConstraintDependents,
                &SiteID,
                &Parent,
            ),
            (With<ModelMarker>, Without<Pending>),
        >,
        Query<&SiteID>,
        Query<&WorkcellProperties>,
    )> = SystemState::new(world);
    let (q_anchors, q_models, q_site_id, q_properties) = state.get(world);

    let mut workcell = Workcell::default();
    match q_properties.get(root) {
        Ok(properties) => {
            workcell.properties = properties.clone();
        }
        Err(_) => {
            return Err(WorkcellGenerationError::InvalidWorkcellEntity(root));
        }
    }

    // Models
    for (name, source, pose, is_static, constraint_dependents, id, parent) in &q_models {
        // Get the parent SiteID
        let parent = match q_site_id.get(parent.get()) {
            Ok(parent) => Some(parent.0),
            Err(_) => None,
        };
        workcell.models.insert(
            id.0,
            Parented {
                parent: parent,
                bundle: Model {
                    name: name.clone(),
                    source: source.clone(),
                    pose: pose.clone(),
                    is_static: is_static.clone(),
                    constraints: constraint_dependents.clone(),
                    marker: ModelMarker,
                },
            },
        );
    }

    // Anchors
    for (anchor, id, parent, constraint) in &q_anchors {
        let parent = match q_site_id.get(parent.get()) {
            Ok(parent) => Some(parent.0),
            Err(_) => None,
        };
        // TODO(luca) is duplication here OK? same information is contained in mesh constraint and
        // anchor
        workcell.frames.insert(
            id.0,
            Parented {
                parent: parent,
                bundle: Frame {
                    anchor: anchor.clone(),
                    marker: FrameMarker,
                }
            },
        );
        if let Some(c) = constraint {
            // Also add a mehs constraint
            workcell.mesh_constraints.insert(
                id.0,
                MeshConstraint {
                    entity: **q_site_id.get(c.entity).unwrap(),
                    element: c.element.clone(),
                    relative_pose: c.relative_pose,
                },
            );
        }
    }

    Ok(workcell)
}

pub fn save_workcell(world: &mut World) {
    let save_events: Vec<_> = world
        .resource_mut::<Events<SaveWorkcell>>()
        .drain()
        .collect();
    for save_event in save_events {
        let path = save_event.to_file;
        println!(
            "Saving to {}",
            path.to_str().unwrap_or("<failed to render??>")
        );
        let f = match std::fs::File::create(path) {
            Ok(f) => f,
            Err(err) => {
                println!("Unable to save file: {err}");
                continue;
            }
        };

        let workcell = match generate_workcell(world, save_event.root) {
            Ok(root) => root,
            Err(err) => {
                println!("Unable to compile workcell: {err}");
                continue;
            }
        };

        dbg!(&workcell);

        match workcell.to_writer(f) {
            Ok(()) => {
                println!("Save successful");
            }
            Err(err) => {
                println!("Save failed: {err}");
            }
        }
    }
}