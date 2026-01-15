use bevy::prelude::Component;

/// Different types of buildings that can be constructed in provinces
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum BuildingType {
    Market,
    Workshop,
    Temple,
    Fort,
    Barracks,
    University,
}

impl BuildingType {
    pub(crate) fn name(&self) -> &str {
        match self {
            BuildingType::Market => "Market",
            BuildingType::Workshop => "Workshop",
            BuildingType::Temple => "Temple",
            BuildingType::Fort => "Fort",
            BuildingType::Barracks => "Barracks",
            BuildingType::University => "University",
        }
    }

    pub(crate) fn cost(&self) -> f32 {
        match self {
            BuildingType::Market => 100.0,
            BuildingType::Workshop => 150.0,
            BuildingType::Temple => 200.0,
            BuildingType::Fort => 300.0,
            BuildingType::Barracks => 250.0,
            BuildingType::University => 400.0,
        }
    }

    pub(crate) fn income_bonus(&self) -> f32 {
        match self {
            BuildingType::Market => 5.0,
            BuildingType::Workshop => 8.0,
            BuildingType::Temple => 3.0,
            BuildingType::Fort => 0.0,
            BuildingType::Barracks => 0.0,
            BuildingType::University => 0.0,
        }
    }

    pub(crate) fn description(&self) -> &str {
        match self {
            BuildingType::Market => "Increases income by 5",
            BuildingType::Workshop => "Increases income by 8",
            BuildingType::Temple => "Increases income by 3",
            BuildingType::Fort => "Province defense (TODO)",
            BuildingType::Barracks => "Troop recruitment (TODO)",
            BuildingType::University => "Technology research (TODO)",
        }
    }

    pub(crate) fn all_types() -> [BuildingType; 6] {
        [
            BuildingType::Market,
            BuildingType::Workshop,
            BuildingType::Temple,
            BuildingType::Fort,
            BuildingType::Barracks,
            BuildingType::University,
        ]
    }
}

/// Component marking a building in a province
#[derive(Component)]
pub(crate) struct Building {
    pub(crate) building_type: BuildingType,
}

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
