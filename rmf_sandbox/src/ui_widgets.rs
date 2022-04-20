use bevy::{
    app::AppExit,
    prelude::*,
    render::{
        camera::{ActiveCamera, Camera3d},
    },
};
use bevy_egui::{egui, EguiContext, EguiPlugin};

use super::camera_controls::{CameraControls, ProjectionMode};
use super::site_map::{SiteMap};


fn egui_ui(
    mut sm: ResMut<SiteMap>,
    mut egui_context: ResMut<EguiContext>,
    mut query: Query<&mut CameraControls>,
    commands: Commands,
    meshes: ResMut<Assets<Mesh>>,
    materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
    mut active_camera_3d: ResMut<ActiveCamera<Camera3d>>,
    mut exit: EventWriter<AppExit>,
) {
    let mut controls = query.single_mut();
    egui::TopBottomPanel::top("top_panel")
        .show(egui_context.ctx_mut(), |ui| {
            ui.vertical(|ui| {

                egui::menu::bar(ui, |ui| {
                    egui::menu::menu_button(ui, "File", |ui| {
                        if ui.button("Load demo").clicked() {
                            sm.load_demo();
                            sm.spawn(commands, meshes, materials, asset_server);
                        }

                        #[cfg(not(target_arch = "wasm32"))]
                        if ui.button("Quit").clicked() {
                            exit.send(AppExit);
                        }
                    });
                });

                ui.horizontal(|ui| {
                    ui.label("[toolbar buttons]");
                    ui.separator();
                    if ui.add(egui::SelectableLabel::new(controls.mode == ProjectionMode::Orthographic, "2D")).clicked() {
                        controls.set_mode(ProjectionMode::Orthographic);
                        active_camera_3d.set(controls.orthographic_camera_entity);
                    }
                    if ui.add(egui::SelectableLabel::new(controls.mode == ProjectionMode::Perspective, "3D")).clicked() {
                        controls.set_mode(ProjectionMode::Perspective);
                        active_camera_3d.set(controls.perspective_camera_entity);
                    }
                });
            });
        });
}

pub struct UIWidgetsPlugin;

impl Plugin for UIWidgetsPlugin{
    fn build(&self, app: &mut App) {
        app.add_plugin(EguiPlugin)
           .add_system(egui_ui);
    }
}