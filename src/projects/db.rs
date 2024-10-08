use crate::areas::db::Entity as Area;
use chrono::NaiveDateTime;
use sea_orm::entity::prelude::*;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "project")]
pub struct Model {
    #[sea_orm(unique)]
    pub name: String,
    pub description: Option<String>,
    pub color: String,
    #[sea_orm(primary_key)]
    pub iterator: i32,
    #[sea_orm(unique)]
    pub id: Uuid,
    pub last_updated: NaiveDateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "Area")]
    Area,
    #[sea_orm(has_many = "crate::instrument_experiments::db::Entity")]
    Instrumentexperiment,
}

impl Related<Area> for Entity {
    fn to() -> RelationDef {
        Relation::Area.def()
    }
}

impl Related<crate::instrument_experiments::db::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Instrumentexperiment.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
