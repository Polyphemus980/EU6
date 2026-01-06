use bevy::prelude::Component;

#[derive(Component)]
pub(crate) struct Building {}

#[derive(Component)]
pub(crate) struct Cost(f32);

/// Component representing income from a single source. Can be added to provinces, building, ....
#[derive(Component)]
pub(crate) struct Income(f32);

impl Income {
    pub(crate) fn new(base_income: f32) -> Self {
        Self(base_income)
    }

    pub(crate) fn get(&self) -> f32 {
        self.0
    }
}
