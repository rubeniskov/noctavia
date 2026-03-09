use nalgebra_glm as glm;

#[derive(Debug, Clone, Copy)]
pub struct OrbitalCamera {
    pub center: glm::Vec3,
    pub radius: f32,
    pub azimuth: f32,
    pub elevation: f32,
}

impl OrbitalCamera {
    pub fn new(center: glm::Vec3, radius: f32) -> Self {
        Self {
            center,
            radius,
            azimuth: 0.0,
            elevation: 0.5,
        }
    }

    pub fn eye_position(&self) -> glm::Vec3 {
        let cos_elev = self.elevation.cos();
        self.center + glm::vec3(
            self.radius * cos_elev * self.azimuth.sin(),
            -self.radius * cos_elev * self.azimuth.cos(),
            self.radius * self.elevation.sin(),
        )
    }

    pub fn view_matrix(&self) -> glm::Mat4 {
        let eye = self.eye_position();
        let up = glm::vec3(0.0, 0.0, 1.0);
        glm::look_at_lh(&eye, &self.center, &up)
    }

    pub fn projection_matrix(&self, aspect: f32) -> glm::Mat4 {
        glm::perspective_lh_zo(aspect, 0.8, 0.1, 1000.0)
    }

    pub fn orbit(&mut self, delta_x: f32, delta_y: f32) {
        self.azimuth += delta_x;
        self.elevation += delta_y;
        self.elevation = self.elevation.clamp(-1.57, 1.57);
    }

    pub fn zoom(&mut self, delta: f32) {
        self.radius -= delta;
        self.radius = self.radius.clamp(0.5, 500.0);
    }
}
