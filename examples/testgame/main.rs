#[macro_use]
extern crate microprofile;

//

use rand::{distributions as distr, distributions::Distribution};

use moleengine::{
    core::{
        self,
        game::{self, Game},
        inputcache::{Key, MouseButton},
        math as m,
    },
    graphics as gx, physics2d as phys,
};

mod player;
mod recipes;

fn main() {
    microprofile::init!();
    microprofile::set_enable_all_groups!(true);

    let game = Game::init(
        winit::window::WindowBuilder::new()
            .with_title("MoleEngine test")
            .with_inner_size(winit::dpi::LogicalSize {
                width: 800.0,
                height: 600.0,
            }),
    );
    let state = State::init(&game.renderer.device);
    game.run(game::LockstepLoop::from_fps(60), state);

    microprofile::shutdown!();
}

//
// Types
//

pub enum StateEnum {
    Playing,
    Paused,
}
pub struct State {
    pub state: StateEnum,
    pub space: MainSpace,
}
impl State {
    fn init(device: &wgpu::Device) -> Self {
        State {
            state: StateEnum::Playing,
            space: load_main_space(device).unwrap(),
        }
    }
}

pub type MainSpace = core::Space<MainSpaceFeatures>;

pub struct MainSpaceFeatures {
    pub tr: core::TransformFeature,
    pub shape: gx::ShapeFeature,
    pub physics: phys::PhysicsFeature,
    pub player: player::PlayerController,
    pub camera: gx::camera::MouseDragCamera,
}

impl core::space::FeatureSet for MainSpaceFeatures {
    fn init(init: core::space::FeatureSetInit) -> Self {
        MainSpaceFeatures {
            tr: core::TransformFeature::new(init),
            shape: gx::ShapeFeature::new(init),
            physics: phys::PhysicsFeature::new(init),
            player: player::PlayerController::new(init),
            camera: gx::camera::MouseDragCamera::new(
                gx::camera::ScalingStrategy::ConstantDisplayArea {
                    width: 8.0,
                    height: 6.0,
                },
            ),
        }
    }

    fn tick(&mut self, mut space: core::SpaceAccess<'_>, game: &Game, dt: f32) {
        microprofile::flip();
        microprofile::scope!("update", "all");
        {
            microprofile::scope!("update", "player");
            self.player
                .tick(space.write(), &game.input, &self.tr, &self.physics);
        }
        {
            microprofile::scope!("update", "physics");
            let grav = phys::forcefield::Gravity(m::Vec2::new(0.0, -9.81));
            let repulsor = phys::forcefield::PointGravity {
                position: m::Point2::new(-1.0, -4.0),
                strength: -14.0,
                falloff: 0.3,
            };
            self.physics.tick(
                space.read(),
                &mut self.tr,
                dt,
                Some(&phys::forcefield::Sum(grav, repulsor)),
            );
        }
    }

    fn draw(&mut self, space: core::SpaceReadAccess<'_>, ctx: &mut gx::RenderContext) {
        microprofile::scope!("render", "all");

        {
            microprofile::scope!("render", "shape");
            self.shape.draw(&space, &self.tr, &self.camera, ctx);
        }
    }
}

impl game::GameState for State {
    fn tick(&mut self, dt: f32, game: &Game) -> Option<()> {
        //
        // State-independent stuff
        //
        if game.input.is_key_pressed(Key::Escape, None) {
            return None;
        }

        // mouse camera

        let camera = &mut self.space.features.camera;
        camera.update(&game.input, game.renderer.window_size().into());
        if (game.input).is_mouse_button_pressed(MouseButton::Middle, Some(0)) {
            camera.transform = m::Transform::identity();
        }

        match self.state {
            //
            // Playing
            //
            StateEnum::Playing => {
                if game.input.is_key_pressed(Key::Space, Some(0)) {
                    self.state = StateEnum::Paused;
                    return Some(());
                }

                if game.input.is_key_pressed(Key::Return, Some(0)) {
                    self.space = load_main_space(&game.renderer.device).unwrap();
                }

                // pool spawning

                let random_pos = || {
                    let mut rng = rand::thread_rng();
                    m::Vec2::new(
                        distr::Uniform::from(-3.0..3.0).sample(&mut rng),
                        distr::Uniform::from(0.0..2.0).sample(&mut rng),
                    )
                };
                let random_angle = || {
                    m::Angle::Deg(distr::Uniform::from(0.0..360.0).sample(&mut rand::thread_rng()))
                };
                let mut rng = rand::thread_rng();
                if game.input.is_key_pressed(Key::S, Some(0)) {
                    self.space.spawn(recipes::DynamicBlock {
                        transform: m::TransformBuilder::new()
                            .with_position(random_pos())
                            .with_rotation(random_angle()),
                        width: distr::Uniform::from(0.6..1.0).sample(&mut rng),
                        height: distr::Uniform::from(0.3..0.8).sample(&mut rng),
                    });
                }
                if game.input.is_key_pressed(Key::T, Some(0)) {
                    self.space.spawn(recipes::Ball {
                        position: random_pos().into(),
                        radius: distr::Uniform::from(0.1..0.4).sample(&mut rng),
                    });
                }

                //

                self.space.tick(game, dt);

                Some(())
            }
            //
            // Paused
            //
            StateEnum::Paused => {
                if game.input.is_key_pressed(Key::Space, Some(0)) {
                    self.state = StateEnum::Playing;
                    return Some(());
                }

                Some(())
            }
        }
    }

    fn draw(&mut self, renderer: &mut gx::Renderer) {
        let mut ctx = renderer.draw_to_window();
        ctx.clear(wgpu::Color {
            r: 0.1,
            g: 0.1,
            b: 0.1,
            a: 1.0,
        });
        self.space.draw(&mut ctx);
        ctx.submit();
    }
}

fn load_main_space(device: &wgpu::Device) -> Option<MainSpace> {
    let mut space = MainSpace::with_capacity(200, device);
    space.create_pool::<recipes::Player>(5).unwrap();
    space.create_pool::<recipes::Ball>(80).unwrap();
    space.create_pool::<recipes::DynamicBlock>(80).unwrap();

    let dir = "./examples/testgame/scenes";

    let file = std::fs::File::open(format!("{}/test.ron", dir)).expect("Failed to open file");
    space
        .read_ron_file::<recipes::Recipes>(file)
        .expect("Failed to load scene");

    Some(space)
}
