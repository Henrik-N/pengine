use crate::{events, input};
use legion::systems::CommandBuffer;
use legion::{Entity, Resources};
use macaw as m;

/// Tau / 4
const FRAC_TAU_4: f32 = std::f32::consts::FRAC_PI_2;

/// Data related to the editor camera
pub struct MainCamera {
    camera: CameraLocationOrientation,
    pub projection: PerspectiveProjection,
    pub controller: CameraController,
    pub uniform_data: CameraUniformData,
}
impl MainCamera {
    pub fn init(config: &wgpu::SurfaceConfiguration) -> Self {
        let camera = CameraLocationOrientation::new(
            (0.0, 5.0, 10.0).into(),
            f32::to_radians(-90.),
            f32::to_radians(-20.),
        );
        let controller = CameraController::new(4.0, 50.0);

        let projection = PerspectiveProjection {
            fov_y: f32::to_radians(45.0),
            aspect: config.width as f32 / config.height as f32,
            z_near: 0.1,
            z_far: 100.0,
        };

        let mut uniform_data = CameraUniformData::new();
        uniform_data.update_view_proj(&camera, &projection);

        Self {
            camera,
            projection,
            controller,
            uniform_data,
        }
    }

    pub fn update(&mut self, dt: std::time::Duration) {
        // update camera data
        self.controller.update_transform(&mut self.camera, dt);
        self.uniform_data
            .update_view_proj(&self.camera, &self.projection);
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct CameraUniformData {
    pub view_proj: m::Mat4,
}
unsafe impl bytemuck::Pod for CameraUniformData {}
unsafe impl bytemuck::Zeroable for CameraUniformData {}

impl CameraUniformData {
    pub fn new() -> Self {
        Self {
            view_proj: m::Mat4::IDENTITY,
        }
    }

    pub fn update_view_proj(
        &mut self,
        camera: &CameraLocationOrientation,
        proj: &PerspectiveProjection,
    ) {
        self.view_proj = proj.perspective_matrix() * camera.view_matrix();
    }
}

pub struct CameraLocationOrientation {
    pub position: m::Vec3,
    yaw: f32,   // rads
    pitch: f32, // rads
}
impl CameraLocationOrientation {
    pub fn new(eye_position: m::Vec3, yaw: f32, pitch: f32) -> Self {
        Self {
            position: eye_position,
            yaw,
            pitch,
        }
    }

    pub fn view_matrix(&self) -> m::Mat4 {
        m::Mat4::look_at_rh(
            self.position,
            self.position + m::vec3(self.yaw.cos(), self.pitch.sin(), self.yaw.sin()).normalize(),
            m::Vec3::Y,
        )
    }
}

pub struct PerspectiveProjection {
    pub fov_y: f32,
    pub aspect: f32,
    pub z_near: f32,
    pub z_far: f32,
}
impl PerspectiveProjection {
    pub fn resize(&mut self, (width, height): (u32, u32)) {
        self.aspect = width as f32 / height as f32;
    }

    pub fn perspective_matrix(&self) -> m::Mat4 {
        m::Mat4::perspective_rh(self.fov_y, self.aspect, self.z_near, self.z_far)
    }
}

pub struct CameraController {
    left_amount: f32,
    right_amount: f32,
    forward_amount: f32,
    backward_amount: f32,
    up_amount: f32,
    down_amount: f32,
    yaw_amount: f32,
    pitch_amount: f32,
    speed: f32,
    sensitivity: f32,
    mouse_key_down: bool,
}
impl CameraController {
    pub fn new(speed: f32, sensitivity: f32) -> Self {
        Self {
            left_amount: 0.0,
            right_amount: 0.0,
            forward_amount: 0.0,
            backward_amount: 0.0,
            up_amount: 0.0,
            down_amount: 0.0,
            yaw_amount: 0.0,
            pitch_amount: 0.0,
            speed,
            sensitivity,
            mouse_key_down: false,
        }
    }

    pub fn on_event(&mut self, event: &events::PenguinEvent) -> bool {
        match event {
            events::PenguinEvent::Input(input_event) => match input_event {
                input::InputEvent::Key(e) => self.process_key_events(e.key, e.state),
                input::InputEvent::MouseMotion(delta) => {
                    if self.mouse_key_down {
                        self.process_mouse_delta_events(delta.0, delta.1);
                    }
                }
            },
            _ => {}
        }

        false
    }

    fn process_mouse_delta_events(&mut self, dx: f64, dy: f64) {
        self.yaw_amount = dx as _;
        self.pitch_amount = dy as _;
    }

    fn process_key_events(&mut self, key: input::Key, state: input::KeyState) {
        use crate::input::Key;

        let amount = if state == crate::input::KeyState::Down {
            1.0
        } else {
            0.0
        };

        match key {
            Key::A | Key::Left => {
                self.left_amount = amount;
            }
            Key::D | Key::Right => {
                self.right_amount = amount;
            }
            Key::W | Key::Up => {
                self.forward_amount = amount;
            }
            Key::S | Key::Down => {
                self.backward_amount = amount;
            }
            Key::E | Key::Space => {
                self.up_amount = amount;
            }
            Key::Q | Key::LControl => {
                self.down_amount = amount;
            }
            Key::LMouseButton => {
                self.mouse_key_down = if state == crate::input::KeyState::Down {
                    true
                } else {
                    false
                };
            }
            _ => {}
        }
    }

    fn update_transform(
        &mut self,
        camera: &mut CameraLocationOrientation,
        dt: std::time::Duration,
    ) {
        let dt = dt.as_secs_f32();

        // Move forwards/backwards and left/right
        let (yaw_sin, yaw_cos) = camera.yaw.sin_cos();
        let forward = m::vec3(yaw_cos, 0.0, yaw_sin).normalize();
        let right = m::vec3(-yaw_sin, 0.0, yaw_cos).normalize();
        camera.position += forward * (self.forward_amount - self.backward_amount) * self.speed * dt;
        camera.position += right * (self.right_amount - self.left_amount) * self.speed * dt;

        // Move up/down (no roll)
        camera.position.y += (self.up_amount - self.down_amount) * self.speed * dt;

        // Rotate
        camera.yaw += f32::to_radians(self.yaw_amount) * self.sensitivity * dt;
        camera.pitch -= f32::to_radians(self.pitch_amount) * self.sensitivity * dt;

        // No acceleration
        self.yaw_amount = 0.0;
        self.pitch_amount = 0.0;

        // clamp pitch and prevent it from going too low/high
        let safe_frac = FRAC_TAU_4 - 0.0001;
        camera.pitch = f32::clamp(camera.pitch, -safe_frac, safe_frac);
    }
}
