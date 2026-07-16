#[derive(Clone, Debug, PartialEq)]
pub struct LandmarkPair {
    pub source_x_um: f64,
    pub source_y_um: f64,
    pub target_x_um: f64,
    pub target_y_um: f64,
}

impl LandmarkPair {
    pub fn new(source_x_um: f64, source_y_um: f64, target_x_um: f64, target_y_um: f64) -> Self {
        Self {
            source_x_um,
            source_y_um,
            target_x_um,
            target_y_um,
        }
    }

    pub(crate) fn is_finite(&self) -> bool {
        self.source_x_um.is_finite()
            && self.source_y_um.is_finite()
            && self.target_x_um.is_finite()
            && self.target_y_um.is_finite()
    }
}
