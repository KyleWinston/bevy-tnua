pub mod component_alterbation;
#[cfg(feature = "egui")]
pub mod plotting;
pub mod tuning;

use std::marker::PhantomData;

#[cfg(feature = "egui")]
use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
#[cfg(feature = "egui")]
use bevy::window::{PresentMode, PrimaryWindow};
#[cfg(feature = "egui")]
use bevy_egui::{egui, EguiContexts, EguiPlugin};
#[cfg(feature = "egui")]
use bevy_tnua::TnuaToggle;

use self::component_alterbation::CommandAlteringSelectors;
#[cfg(feature = "egui")]
use self::plotting::{make_update_plot_data_system, plot_source_rolling_update};

use tuning::UiTunable;

pub struct DemoUi<C: Component + UiTunable> {
    _phantom: PhantomData<C>,
}

impl<C: Component + UiTunable> Default for DemoUi<C> {
    fn default() -> Self {
        Self {
            _phantom: Default::default(),
        }
    }
}

impl<C: Component + UiTunable> Plugin for DemoUi<C> {
    fn build(&self, app: &mut App) {
        #[cfg(feature = "egui")]
        app.add_plugins(EguiPlugin);
        app.insert_resource(DemoUiPhysicsBackendActive(true));
        app.add_systems(Update, apply_selectors);
        #[cfg(feature = "egui")]
        app.add_systems(Update, ui_system::<C>);
        #[cfg(feature = "egui")]
        app.add_systems(Update, plot_source_rolling_update);
        #[cfg(feature = "egui")]
        app.add_plugins(FrameTimeDiagnosticsPlugin);

        #[cfg(feature = "egui")]
        {
            #[cfg(feature = "rapier2d")]
            app.add_systems(
                Update,
                make_update_plot_data_system(|velocity: &bevy_rapier2d::prelude::Velocity| {
                    velocity.linvel.extend(0.0)
                }),
            );
            #[cfg(feature = "rapier3d")]
            app.add_systems(
                Update,
                make_update_plot_data_system(|velocity: &bevy_rapier3d::prelude::Velocity| {
                    velocity.linvel
                }),
            );
            #[cfg(feature = "xpbd2d")]
            app.add_systems(
                Update,
                make_update_plot_data_system(
                    |velocity: &bevy_xpbd_2d::components::LinearVelocity| velocity.extend(0.0),
                ),
            );
            #[cfg(feature = "xpbd3d")]
            app.add_systems(
                Update,
                make_update_plot_data_system(
                    |velocity: &bevy_xpbd_3d::components::LinearVelocity| **velocity,
                ),
            );
        }

        app.add_systems(Update, update_physics_active_from_ui);
    }
}

// NOTE: The demos are responsible for updating the physics backend
#[derive(Resource)]
pub struct DemoUiPhysicsBackendActive(pub bool);

#[derive(Component)]
pub struct TrackedEntity(pub String);

fn apply_selectors(
    mut query: Query<(Entity, &mut CommandAlteringSelectors)>,
    mut commands: Commands,
) {
    for (entity, mut command_altering_selectors) in query.iter_mut() {
        command_altering_selectors.apply_set_to(&mut commands, entity);
    }
}

#[cfg(feature = "egui")]
#[allow(clippy::type_complexity)]
fn ui_system<C: Component + UiTunable>(
    mut egui_context: EguiContexts,
    mut physics_backend_active: ResMut<DemoUiPhysicsBackendActive>,
    mut query: Query<(
        Entity,
        &TrackedEntity,
        &plotting::PlotSource,
        &mut TnuaToggle,
        Option<&mut C>,
        Option<&mut CommandAlteringSelectors>,
    )>,
    mut commands: Commands,
    mut primary_window_query: Query<&mut Window, With<PrimaryWindow>>,
    diagnostics_store: Res<DiagnosticsStore>,
) {
    let Ok(mut primary_window) = primary_window_query.get_single_mut() else {
        return;
    };
    let mut egui_window = egui::Window::new("Tnua");
    if !primary_window.cursor.visible {
        egui_window = egui::Window::new("Tnua")
            .interactable(false)
            .movable(false)
            .resizable(false);
    }
    egui_window.show(egui_context.ctx_mut(), |ui| {
        egui::ComboBox::from_label("Present Mode (picking unsupported mode will crash the demo)")
            .selected_text(format!("{:?}", primary_window.present_mode))
            .show_ui(ui, |ui| {
                let present_mode = &mut primary_window.present_mode;
                ui.selectable_value(present_mode, PresentMode::AutoVsync, "AutoVsync");
                ui.selectable_value(present_mode, PresentMode::AutoNoVsync, "AutoNoVsync");
                ui.selectable_value(present_mode, PresentMode::Fifo, "Fifo");
                ui.selectable_value(present_mode, PresentMode::FifoRelaxed, "FifoRelaxed");
                ui.selectable_value(present_mode, PresentMode::Immediate, "Immediate");
                ui.selectable_value(present_mode, PresentMode::Mailbox, "Mailbox");
            });
        for (diagnostic_path, range) in [
            (FrameTimeDiagnosticsPlugin::FPS, 0.0..120.0),
            (FrameTimeDiagnosticsPlugin::FRAME_TIME, 0.0..50.0),
        ] {
            if let Some(diagnostic) = diagnostics_store.get(&diagnostic_path) {
                if let Some(value) = diagnostic.smoothed() {
                    ui.add(
                        egui::widgets::ProgressBar::new((value as f32 - range.start) / (range.end - range.start))
                        .text(format!("{}: {:.0}", diagnostic_path, value))
                    );
                }
            }
        }
        egui::CollapsingHeader::new("Controls:")
            .default_open(false)
            .show(ui, |ui| {
                ui.label("Move with the arrow keys or WASD");
                ui.label("Left click to toggle mouse-controlled camera (shooter only)");
                ui.label("Jump with Spacebar (Also with the up arrow also works in 2D)");
                ui.label("Crouch or fall through pink platforms with Ctrl (Also with the down arrow key in 2D)");
                ui.label("Turn in place with Alt (only in 3D)");
                ui.label("Dash with Shift (while moving in a direction)");
            });
        ui.checkbox(&mut physics_backend_active.0, "Physics Backend Enabled");
        for (
            entity,
            TrackedEntity(name),
            plot_source,
            mut tnua_toggle,
            mut tunable,
            command_altering_selectors,
        ) in query.iter_mut()
        {
            egui::CollapsingHeader::new(name)
                .default_open(false)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            egui::ComboBox::from_label("Toggle Tnua")
                                .selected_text(format!("{:?}", tnua_toggle.as_ref()))
                                .show_ui(ui, |ui| {
                                    for option in [
                                        TnuaToggle::Disabled,
                                        TnuaToggle::SenseOnly,
                                        TnuaToggle::Enabled,
                                    ] {
                                        let label = format!("{:?}", option);
                                        ui.selectable_value(tnua_toggle.as_mut(), option, label);
                                    }
                                });

                            if let Some(tunable) = tunable.as_mut() {
                                tunable.tune(ui);
                            }

                            if let Some(mut command_altering_selectors) = command_altering_selectors
                            {
                                command_altering_selectors.show_ui(ui, &mut commands, entity);
                            }
                        });
                        ui.vertical(|ui| {
                            plot_source.show(entity, ui);
                        });
                    });
                });
        }
    });
}

fn update_physics_active_from_ui(
    setting_from_ui: Res<DemoUiPhysicsBackendActive>,
    #[cfg(feature = "rapier2d")] mut config_rapier2d: Option<
        ResMut<bevy_rapier2d::plugin::RapierConfiguration>,
    >,
    #[cfg(feature = "rapier3d")] mut config_rapier3d: Option<
        ResMut<bevy_rapier3d::plugin::RapierConfiguration>,
    >,
    #[cfg(feature = "xpbd2d")] mut physics_time_xpbd2d: Option<
        ResMut<Time<bevy_xpbd_2d::plugins::setup::Physics>>,
    >,
    #[cfg(feature = "xpbd3d")] mut physics_time_xpbd3d: Option<
        ResMut<Time<bevy_xpbd_3d::plugins::setup::Physics>>,
    >,
) {
    #[cfg(feature = "rapier2d")]
    if let Some(config) = config_rapier2d.as_mut() {
        config.physics_pipeline_active = setting_from_ui.0;
    }
    #[cfg(feature = "rapier3d")]
    if let Some(config) = config_rapier3d.as_mut() {
        config.physics_pipeline_active = setting_from_ui.0;
    }
    #[cfg(feature = "xpbd2d")]
    if let Some(physics_time) = physics_time_xpbd2d.as_mut() {
        use bevy_xpbd_2d::plugins::setup::PhysicsTime;
        if setting_from_ui.0 {
            physics_time.unpause();
        } else {
            physics_time.pause();
        }
    }
    #[cfg(feature = "xpbd3d")]
    if let Some(physics_time) = physics_time_xpbd3d.as_mut() {
        use bevy_xpbd_3d::plugins::setup::PhysicsTime;
        if setting_from_ui.0 {
            physics_time.unpause();
        } else {
            physics_time.pause();
        }
    }
}
