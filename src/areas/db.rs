use crate::plots::db::Entity as Plot;
use crate::projects::db::Entity as Project;
use crate::sensors::db::Entity as Sensor;
use crate::soil::profiles::db::Entity as SoilProfile;
use crate::transects::db::Entity as Transect;

use chrono::NaiveDateTime;
use sea_orm::entity::prelude::*;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "area")]
pub struct Model {
    pub name: Option<String>,
    pub description: Option<String>,
    pub project_id: Uuid,
    #[sea_orm(primary_key)]
    pub iterator: i32,
    #[sea_orm(unique)]
    pub id: Uuid,
    pub last_updated: NaiveDateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "Plot")]
    Plot,
    #[sea_orm(
        belongs_to = "Project",
        from = "Column::ProjectId",
        to = "crate::projects::db::Column::Id",
        on_update = "NoAction",
        on_delete = "NoAction"
    )]
    Project,
    #[sea_orm(has_many = "Sensor")]
    Sensor,
    #[sea_orm(has_many = "SoilProfile")]
    Soilprofile,
    #[sea_orm(has_many = "Transect")]
    Transect,
}

impl Related<Plot> for Entity {
    fn to() -> RelationDef {
        Relation::Plot.def()
    }
}

impl Related<Project> for Entity {
    fn to() -> RelationDef {
        Relation::Project.def()
    }
}

impl Related<Sensor> for Entity {
    fn to() -> RelationDef {
        Relation::Sensor.def()
    }
}

impl Related<SoilProfile> for Entity {
    fn to() -> RelationDef {
        Relation::Soilprofile.def()
    }
}

impl Related<Transect> for Entity {
    fn to() -> RelationDef {
        Relation::Transect.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
