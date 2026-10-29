#[derive(Clone, Copy)]
pub(crate) struct TimeStep {
    pub(crate) completed_steps: u32,
    pub(crate) time: f64,
    pub(crate) dt: f64,
    pub(crate) should_record: bool,
}

pub(crate) struct TimeGrid {
    final_time: f64,
    time_step: f64,
    planned_steps: u32,
    record_every_steps: u32,
    completed_steps: u32,
    time: f64,
}

impl TimeGrid {
    pub(crate) fn new(
        final_time: f64,
        time_step: f64,
        planned_steps: u32,
        record_every_steps: u32,
    ) -> Self {
        Self {
            final_time,
            time_step,
            planned_steps,
            record_every_steps,
            completed_steps: 0,
            time: 0.0,
        }
    }
}

impl Iterator for TimeGrid {
    type Item = TimeStep;

    fn next(&mut self) -> Option<Self::Item> {
        if self.completed_steps >= self.planned_steps {
            return None;
        }
        let completed_steps = self.completed_steps + 1;
        let time = if completed_steps == self.planned_steps {
            self.final_time
        } else {
            f64::from(completed_steps) * self.time_step
        };
        let step = TimeStep {
            completed_steps,
            time,
            dt: time - self.time,
            should_record: completed_steps.is_multiple_of(self.record_every_steps)
                || completed_steps == self.planned_steps,
        };
        self.completed_steps = completed_steps;
        self.time = time;
        Some(step)
    }
}

pub(crate) fn retained_snapshot_count(steps: u32, record_every_steps: u32) -> u64 {
    1 + u64::from(steps / record_every_steps) + u64::from(!steps.is_multiple_of(record_every_steps))
}
