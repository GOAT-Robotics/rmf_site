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

use crate::{
    Anchor, Angle, Category, Door, DoorType, Level, LiftCabin, Pose, Rotation, Side, Site, Swing, NameInSite, DoorMarker,
};
use sdformat_rs::*;
use std::collections::BTreeMap;

#[derive(Debug)]
pub enum SdfConversionError {
    /// An asset that can't be converted to an sdf world was found.
    UnsupportedAssetType,
    /// Entity referenced a non existing anchor.
    BrokenAnchorReference(u32),
    /// Entity referenced a non existing level.
    BrokenLevelReference(u32),
}

impl Pose {
    fn to_sdf(&self, elevation: f32) -> SdfPose {
        let p = &self.trans;
        let r = match self.rot {
            Rotation::Yaw(angle) => format!("0 0 {}", angle.radians()),
            Rotation::EulerExtrinsicXYZ(rpy) => format!(
                "{} {} {}",
                rpy[0].radians(),
                rpy[1].radians(),
                rpy[2].radians()
            ),
            Rotation::Quat(quat) => format!("{} {} {} {}", quat[0], quat[1], quat[2], quat[3]),
        };
        SdfPose {
            data: format!("{} {} {} {}", p[0], p[1], p[2] + elevation, r),
            ..Default::default()
        }
    }
}

fn make_sdf_door_link(door_name: &str, link_name: &str) -> SdfLink {
    let door_mass = 50.0;
    SdfLink {
        name: link_name.to_string(),
        collision: vec![SdfCollision {
            name: format!("{}_collision", link_name),
            geometry: SdfGeometry::Mesh(SdfMeshShape {
                uri: format!("meshes/{}_{}.glb", door_name, link_name),
                ..Default::default()
            }),
            surface: Some(SdfSurface {
                contact: Some(SdfSurfaceContact {
                    collide_bitmask: Some("0x02".into()),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        }],
        visual: vec![SdfVisual {
            name: format!("{}_visual", link_name),
            geometry: SdfGeometry::Mesh(SdfMeshShape {
                uri: format!("meshes/{}_{}.glb", door_name, link_name),
                ..Default::default()
            }),
            ..Default::default()
        }],
        // TODO(luca) calculate inertia based on door properties
        inertial: Some(SdfInertial {
            mass: Some(door_mass),
            inertia: Some(SdfInertialInertia {
                ixx: 20.0,
                iyy: 20.0,
                izz: 5.0,
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    }
}

impl Door<u32> {
    pub fn to_sdf(
        &self,
        left_anchor: Anchor,
        right_anchor: Anchor,
        elevation: f32,
    ) -> Result<SdfModel, SdfConversionError> {
        let door_mass = 50.0;
        let left_trans = left_anchor.translation_for_category(Category::Door);
        let right_trans = right_anchor.translation_for_category(Category::Door);
        let center = [
            (left_trans[0] + right_trans[0]) / 2.0,
            (left_trans[1] + right_trans[1]) / 2.0,
        ];
        let dx = left_trans[0] - right_trans[0];
        let dy = left_trans[1] - right_trans[1];
        let door_length = (dx * dx + dy * dy).sqrt();
        let yaw = -dx.atan2(dy);
        let labels = match self.kind {
            DoorType::SingleSliding(_) | DoorType::SingleSwing(_) | DoorType::Model(_) => {
                Vec::from(["body"])
            }
            DoorType::DoubleSliding(_) | DoorType::DoubleSwing(_) => Vec::from(["right", "left"]),
        };
        let mut plugin = SdfPlugin {
            name: "register_component".into(),
            filename: "libregister_component.so".into(),
            ..Default::default()
        };
        let mut component = XmlElement {
            name: "component".into(),
            ..Default::default()
        };
        let mut door_plugin_inner = XmlElement {
            name: "door".into(),
            ..Default::default()
        };
        component.attributes.insert("name".into(), "Door".into());
        let mut component_data = ElementMap::default();
        door_plugin_inner
            .attributes
            .insert("name".to_string(), self.name.0.clone());
        let mut door_model = SdfModel {
            name: self.name.0.clone(),
            pose: Some(
                Pose {
                    trans: [center[0], center[1], elevation],
                    rot: Rotation::Yaw(Angle::Rad(yaw)),
                }
                .to_sdf(0.0),
            ),
            r#static: Some(false),
            ..Default::default()
        };
        for label in labels.iter() {
            door_model
                .link
                .push(make_sdf_door_link(&self.name.0, label));
        }
        let mut door_motion_params = vec![];
        let joints = match &self.kind {
            DoorType::SingleSliding(door) => {
                door_plugin_inner
                    .attributes
                    .insert("type".into(), "SlidingDoor".into());
                door_plugin_inner
                    .attributes
                    .insert("left_joint_name".into(), "empty_joint".into());
                door_plugin_inner
                    .attributes
                    .insert("right_joint_name".into(), "joint".into());
                door_motion_params.push(("v_max_door", "0.2"));
                door_motion_params.push(("a_max_door", "0.2"));
                door_motion_params.push(("a_nom_door", "0.08"));
                door_motion_params.push(("dx_min_door", "0.001"));
                door_motion_params.push(("f_max_door", "100.0"));
                let pose = Pose {
                    trans: [0.0, (door_length / 2.0) * door.towards.sign(), 1.25],
                    ..Default::default()
                }
                .to_sdf(0.0);
                vec![SdfJoint {
                    name: "joint".into(),
                    parent: "world".into(),
                    child: "body".into(),
                    r#type: "prismatic".into(),
                    pose: Some(pose),
                    axis: Some(SdfJointAxis {
                        xyz: Vector3d::new(0.0, door.towards.sign().into(), 0.0),
                        limit: SdfJointaxisLimit {
                            lower: 0.0,
                            upper: door_length as f64,
                            ..Default::default()
                        },
                        ..Default::default()
                    }),
                    ..Default::default()
                }]
            }
            DoorType::SingleSwing(door) => {
                door_plugin_inner
                    .attributes
                    .insert("type".into(), "SwingDoor".into());
                door_motion_params.push(("v_max_door", "0.5"));
                door_motion_params.push(("a_max_door", "0.3"));
                door_motion_params.push(("a_nom_door", "0.15"));
                door_motion_params.push(("dx_min_door", "0.01"));
                door_motion_params.push(("f_max_door", "500.0"));
                let side = door.pivot_on.sign() as f64;
                let (open, z) = match door.swing {
                    Swing::Forward(angle) => (angle.radians() as f64, side),
                    Swing::Backward(angle) => (angle.radians() as f64, -side),
                    // Only use the forward position for double doors
                    Swing::Both { forward, backward } => (forward.radians() as f64, side),
                };
                let lower = 0.0;
                let upper = open.abs();
                let pose = Pose {
                    trans: [0.0, (door_length / 2.0) * door.pivot_on.sign(), 1.25],
                    ..Default::default()
                }
                .to_sdf(0.0);
                let (left_joint_name, right_joint_name) = ("empty_joint", "joint");
                door_plugin_inner
                    .attributes
                    .insert("left_joint_name".into(), left_joint_name.into());
                door_plugin_inner
                    .attributes
                    .insert("right_joint_name".into(), right_joint_name.into());
                vec![SdfJoint {
                    name: "joint".into(),
                    parent: "world".into(),
                    child: "body".into(),
                    r#type: "revolute".into(),
                    axis: Some(SdfJointAxis {
                        xyz: Vector3d::new(0.0, 0.0, z),
                        limit: SdfJointaxisLimit {
                            lower,
                            upper,
                            ..Default::default()
                        },
                        ..Default::default()
                    }),
                    pose: Some(pose),
                    ..Default::default()
                }]
            }
            DoorType::DoubleSliding(door) => {
                door_plugin_inner
                    .attributes
                    .insert("type".into(), "DoubleSlidingDoor".into());
                door_plugin_inner
                    .attributes
                    .insert("left_joint_name".into(), "left_joint".into());
                door_plugin_inner
                    .attributes
                    .insert("right_joint_name".into(), "right_joint".into());
                door_motion_params.push(("v_max_door", "0.2"));
                door_motion_params.push(("a_max_door", "0.2"));
                door_motion_params.push(("a_nom_door", "0.08"));
                door_motion_params.push(("dx_min_door", "0.001"));
                door_motion_params.push(("f_max_door", "100.0"));
                let right_pose = Pose {
                    trans: [0.0, -door_length / 2.0, 1.25],
                    ..Default::default()
                }
                .to_sdf(0.0);
                let left_pose = Pose {
                    trans: [0.0, door_length / 2.0, 1.25],
                    ..Default::default()
                }
                .to_sdf(0.0);
                let left_length =
                    (door.left_right_ratio / (1.0 + door.left_right_ratio)) * door_length;
                let right_length = door_length - left_length;
                vec![
                    SdfJoint {
                        name: "right_joint".into(),
                        parent: "world".into(),
                        child: "right".into(),
                        r#type: "prismatic".into(),
                        pose: Some(right_pose),
                        axis: Some(SdfJointAxis {
                            xyz: Vector3d::new(0.0, -1.0, 0.0),
                            limit: SdfJointaxisLimit {
                                lower: 0.0,
                                upper: right_length as f64,
                                ..Default::default()
                            },
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                    SdfJoint {
                        name: "left_joint".into(),
                        parent: "world".into(),
                        child: "left".into(),
                        r#type: "prismatic".into(),
                        pose: Some(left_pose),
                        axis: Some(SdfJointAxis {
                            xyz: Vector3d::new(0.0, -1.0, 0.0),
                            limit: SdfJointaxisLimit {
                                lower: -left_length as f64,
                                upper: 0.0,
                                ..Default::default()
                            },
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                ]
            }
            DoorType::DoubleSwing(door) => {
                door_plugin_inner
                    .attributes
                    .insert("type".into(), "DoubleSwingDoor".into());
                door_plugin_inner
                    .attributes
                    .insert("left_joint_name".into(), "left_joint".into());
                door_plugin_inner
                    .attributes
                    .insert("right_joint_name".into(), "right_joint".into());
                door_motion_params.push(("v_max_door", "0.5"));
                door_motion_params.push(("a_max_door", "0.3"));
                door_motion_params.push(("a_nom_door", "0.15"));
                door_motion_params.push(("dx_min_door", "0.01"));
                door_motion_params.push(("f_max_door", "500.0"));
                let (open, z) = match door.swing {
                    Swing::Forward(angle) => (angle.radians() as f64, -1.0),
                    Swing::Backward(angle) => (angle.radians() as f64, 1.0),
                    // Only use the forward position for double doors
                    Swing::Both { forward, backward } => (forward.radians() as f64, -1.0),
                };
                let lower = 0.0;
                let upper = open.abs();
                let right_pose = Pose {
                    trans: [0.0, -door_length / 2.0, 1.25],
                    ..Default::default()
                }
                .to_sdf(0.0);
                let left_pose = Pose {
                    trans: [0.0, door_length / 2.0, 1.25],
                    ..Default::default()
                }
                .to_sdf(0.0);
                vec![
                    SdfJoint {
                        name: "right_joint".into(),
                        parent: "world".into(),
                        child: "right".into(),
                        r#type: "revolute".into(),
                        axis: Some(SdfJointAxis {
                            xyz: Vector3d::new(0.0, 0.0, z),
                            limit: SdfJointaxisLimit {
                                lower: 0.0,
                                upper,
                                ..Default::default()
                            },
                            ..Default::default()
                        }),
                        pose: Some(right_pose),
                        ..Default::default()
                    },
                    SdfJoint {
                        name: "left_joint".into(),
                        parent: "world".into(),
                        child: "left".into(),
                        r#type: "revolute".into(),
                        axis: Some(SdfJointAxis {
                            xyz: Vector3d::new(0.0, 0.0, z),
                            limit: SdfJointaxisLimit {
                                lower: -upper,
                                upper: 0.0,
                                ..Default::default()
                            },
                            ..Default::default()
                        }),
                        pose: Some(left_pose),
                        ..Default::default()
                    },
                ]
            }
            DoorType::Model(_) => {
                // Unimplemented! Use a fixed joint for now
                let pose = Pose {
                    trans: [0.0, door_length / 2.0, 1.25],
                    ..Default::default()
                }
                .to_sdf(0.0);
                vec![SdfJoint {
                    name: "joint".into(),
                    parent: "world".into(),
                    child: "body".into(),
                    r#type: "fixed".into(),
                    pose: Some(pose),
                    ..Default::default()
                }]
            }
        };
        door_motion_params.push(("ros_interface", "true"));
        door_model.joint.extend(joints);
        for (name, value) in door_motion_params.into_iter() {
            component_data.push(XmlElement {
                name: name.into(),
                data: ElementData::String(value.to_string()),
                ..Default::default()
            });
        }
        component_data.push(door_plugin_inner);
        component.data = ElementData::Nested(component_data);
        plugin.elements.push(component);
        door_model.plugin = vec![plugin];
        Ok(door_model)
    }
}

impl Site {
    pub fn to_sdf(&self) -> Result<SdfRoot, SdfConversionError> {
        let get_anchor = |id: u32| -> Result<Anchor, SdfConversionError> {
            self.anchors.get(&id)
                .or_else(|| self.levels.values().find_map(|l| l.anchors.get(&id)))
                .ok_or(SdfConversionError::BrokenAnchorReference(id))
                .cloned()
        };
        let mut models = Vec::new();
        for level in self.levels.values() {
            // Floors walls and static models are included in the level mesh
            models.push(SdfModel {
                name: format!("level_{}", level.properties.name.0),
                r#static: Some(true),
                link: vec![SdfLink {
                    name: "link".into(),
                    collision: vec![SdfCollision {
                        name: "collision".into(),
                        geometry: SdfGeometry::Mesh(SdfMeshShape {
                            uri: format!("meshes/level_{}_collision.glb", level.properties.name.0),
                            ..Default::default()
                        }),
                        surface: Some(SdfSurface {
                            contact: Some(SdfSurfaceContact {
                                collide_bitmask: Some("0x01".into()),
                                ..Default::default()
                            }),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }],
                    visual: vec![SdfVisual {
                        name: "visual".into(),
                        geometry: SdfGeometry::Mesh(SdfMeshShape {
                            uri: format!("meshes/level_{}_visual.glb", level.properties.name.0),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            });
            // Now add all the doors
            for door in level.doors.values() {
                let left_anchor = get_anchor(door.anchors.left())?;
                let right_anchor = get_anchor(door.anchors.right())?;
                models.push(door.to_sdf(
                    left_anchor,
                    right_anchor,
                    level.properties.elevation.0,
                )?);
            }
        }
        for lift in self.lifts.values() {
            // Cabin
            let LiftCabin::Rect(ref cabin) = lift.properties.cabin else {
                continue;
            };
            let center = cabin.aabb().center;
            let left_anchor = get_anchor(lift.properties.reference_anchors.left())?;
            let right_anchor = get_anchor(lift.properties.reference_anchors.right())?;
            let left_trans = left_anchor.translation_for_category(Category::Level);
            let right_trans = right_anchor.translation_for_category(Category::Level);
            let midpoint = [
                (left_trans[0] + right_trans[0]) / 2.0,
                (left_trans[1] + right_trans[1]) / 2.0,
            ];
            let pose = Pose {
                trans: [midpoint[0] + center.x, midpoint[1] + center.y, 0.0],
                ..Default::default()
            };
            let mut plugin = SdfPlugin {
                name: "lift".into(),
                filename: "liblift.so".into(),
                ..Default::default()
            };
            let mut component = XmlElement {
                name: "component".into(),
                ..Default::default()
            };
            component.attributes.insert("name".into(), "Lift".into());
            let mut component_data = ElementMap::default();
            let mut elements = vec![];
            let lift_name = &lift.properties.name.0;
            elements.push(("lift_name", lift_name.clone()));
            // TODO(luca) remove unwrap here for missing initial level
            let initial_level = lift
                .properties
                .initial_level
                .0
                .and_then(|id| self.levels.get(&id))
                .map(|level| level.properties.name.0.clone())
                .unwrap();
            elements.push(("initial_level", initial_level));
            elements.push(("v_max_cabin", "2.0".to_string()));
            elements.push(("a_max_cabin", "1.2".to_string()));
            elements.push(("a_nom_cabin", "1.0".to_string()));
            elements.push(("dx_min_cabin", "0.001".to_string()));
            elements.push(("f_max_cabin", "25323.0".to_string()));
            elements.push(("cabin_joint_name", "cabin_joint".to_string()));
            let mut levels: BTreeMap<u32, ElementMap> = BTreeMap::new();
            //let mut lift_models = Vec::new();
            for (door_idx, door) in lift.cabin_doors.iter() {
                let cabin_door_name = format!("CabinDoor_{}_door_{}", lift_name, door_idx);
                // Create a dummy cabin door first
                let dummy_cabin = Door {
                    anchors: door.reference_anchors.clone(),
                    name: NameInSite(cabin_door_name.clone()),
                    kind: door.kind.clone(),
                    marker: DoorMarker,
                };
                // TODO(luca) remove unwrap here
                let left_anchor = lift.cabin_anchors.get(&door.reference_anchors.left()).unwrap().clone();
                let right_anchor = lift.cabin_anchors.get(&door.reference_anchors.right()).unwrap().clone();
                // TODO(luca) do this when nested models are supported
                //lift_models.push(dummy_cabin.to_sdf(
                models.push(dummy_cabin.to_sdf(
                    left_anchor,
                    right_anchor,
                    0.0
                    //level.properties.elevation.0,
                )?);
                for visit in door.visits.0.iter() {
                    let level = self
                        .levels
                        .get(visit)
                        .ok_or(SdfConversionError::BrokenLevelReference(*visit))?;
                    let shaft_door_name = format!("ShaftDoor_{}_{}_door_{}", level.properties.name.0, lift_name, door_idx);
                    // TODO(luca) proper pose for shaft doors
                    let dummy_shaft  = Door {
                        anchors: door.reference_anchors.clone(),
                        name: NameInSite(shaft_door_name.clone()),
                        kind: door.kind.clone(),
                        marker: DoorMarker,
                    };
                    let left_anchor = lift.cabin_anchors.get(&door.reference_anchors.left()).unwrap().clone();
                    let right_anchor = lift.cabin_anchors.get(&door.reference_anchors.right()).unwrap().clone();
                    models.push(dummy_shaft.to_sdf(
                        left_anchor,
                        right_anchor,
                        level.properties.elevation.0,
                    )?);
                    let mut level = levels.entry(*visit).or_default();
                    let element = XmlElement {
                        name: "door_pair".into(),
                        attributes: [
                            ("cabin_door".to_string(), cabin_door_name.clone()),
                            ("shaft_door".to_string(), shaft_door_name.clone()),
                        ]
                        .into(),
                        ..Default::default()
                    };
                    level.push(element);
                }
            }
            for (key, door_pairs) in levels.into_iter() {
                let level = self
                    .levels
                    .get(&key)
                    .ok_or(SdfConversionError::BrokenLevelReference(key))?;
                plugin.elements.push(XmlElement {
                    name: "floor".into(),
                    attributes: [
                        ("name".to_string(), level.properties.name.0.clone()),
                        (
                            "elevation".to_string(),
                            level.properties.elevation.0.to_string(),
                        ),
                    ]
                    .into(),
                    data: ElementData::Nested(door_pairs),
                    ..Default::default()
                });
            }
            for (name, value) in elements.into_iter() {
                component_data.push(XmlElement {
                    name: name.into(),
                    data: ElementData::String(value),
                    ..Default::default()
                });
            }
            component.data = ElementData::Nested(component_data);
            plugin.elements.push(component);
            models.push(SdfModel {
                name: lift.properties.name.0.clone(),
                r#static: Some(lift.properties.is_static.0),
                pose: Some(pose.to_sdf(0.0)),
                link: vec![SdfLink {
                    name: "link".into(),
                    collision: vec![SdfCollision {
                        name: "collision".into(),
                        geometry: SdfGeometry::Mesh(SdfMeshShape {
                            uri: format!("meshes/{}.glb", lift.properties.name.0),
                            ..Default::default()
                        }),
                        surface: Some(SdfSurface {
                            contact: Some(SdfSurfaceContact {
                                collide_bitmask: Some("0x02".into()),
                                ..Default::default()
                            }),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }],
                    visual: vec![SdfVisual {
                        name: "visual".into(),
                        geometry: SdfGeometry::Mesh(SdfMeshShape {
                            uri: format!("meshes/{}.glb", lift.properties.name.0),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                //model: lift_models,
                //plugin: vec![plugin],
                ..Default::default()
            });
        }

        let sun = SdfLight {
            name: "sun".into(),
            r#type: "directional".into(),
            cast_shadows: Some(true),
            diffuse: Some("1 1 1 1".into()),
            pose: Some(Pose::default().to_sdf(10.0)),
            specular: Some("0.2 0.2 0.2 1".into()),
            attenuation: Some(SdfLightAttenuation {
                range: 1000.0,
                constant: Some(0.09),
                linear: Some(0.001),
                quadratic: Some(0.001),
            }),
            direction: Vector3d::new(-0.5, 0.1, -0.9),
            ..Default::default()
        };
        let lift_plugin = SdfPlugin {
            name: "lift".into(),
            filename: "liblift.so".into(),
            ..Default::default()
        };
        let door_plugin = SdfPlugin {
            name: "door".into(),
            filename: "libdoor.so".into(),
            ..Default::default()
        };
        let physics_plugin = SdfPlugin {
            name: "gz::sim::systems::Physics".into(),
            filename: "libgz-sim-physics-system.so".into(),
            ..Default::default()
        };
        let user_commands_plugin = SdfPlugin {
            name: "gz::sim::systems::UserCommands".into(),
            filename: "libgz-sim-user-commands-system.so".into(),
            ..Default::default()
        };
        let scene_broadcaster_plugin = SdfPlugin {
            name: "gz::sim::systems::SceneBroadcaster".into(),
            filename: "libgz-sim-scene-broadcaster-system.so".into(),
            ..Default::default()
        };
        Ok(SdfRoot {
            version: "1.7".to_string(),
            world: vec![SdfWorld {
                name: self.properties.name.0.clone(),
                model: models,
                atmosphere: SdfAtmosphere {
                    r#type: "adiabatic".to_string(),
                    ..Default::default()
                },
                scene: SdfScene {
                    ambient: "1 1 1".to_string(),
                    background: "0.8 0.8 0.8".to_string(),
                    ..Default::default()
                },
                light: vec![sun],
                plugin: vec![physics_plugin, user_commands_plugin, scene_broadcaster_plugin, door_plugin, lift_plugin],
                ..Default::default()
            }],
            ..Default::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::legacy::building_map::BuildingMap;

    #[test]
    fn serde_roundtrip() {
        let data = std::fs::read("../assets/demo_maps/hotel.building.yaml").unwrap();
        let map = BuildingMap::from_bytes(&data).unwrap();
        let site = map.to_site().unwrap();
        // Convert to an sdf
        let sdf = site.to_sdf().unwrap();
        dbg!(&sdf);
        let config = yaserde::ser::Config {
            perform_indent: true,
            write_document_declaration: true,
            ..Default::default()
        };
        let s = yaserde::ser::to_string_with_config(&sdf, &config).unwrap();
        println!("{}", s);
        std::fs::write("test.sdf", s);
        panic!();
    }
}
