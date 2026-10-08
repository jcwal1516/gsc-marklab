#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ComponentSummary {
    pub component_count: usize,
    pub largest_fraction: f64,
}

impl ComponentSummary {
    pub fn from_component_ids(component_ids: &[u32]) -> Self {
        if component_ids.is_empty() {
            return Self {
                component_count: 0,
                largest_fraction: 0.0,
            };
        }

        let mut sorted = component_ids.to_vec();
        sorted.sort_unstable();

        let mut component_count = 0;
        let mut largest = 0;
        let mut current_id = sorted[0];
        let mut current_count = 0;

        for id in sorted {
            if id == current_id {
                current_count += 1;
            } else {
                component_count += 1;
                largest = largest.max(current_count);
                current_id = id;
                current_count = 1;
            }
        }
        component_count += 1;
        largest = largest.max(current_count);

        Self {
            component_count,
            largest_fraction: largest as f64 / component_ids.len() as f64,
        }
    }
}
