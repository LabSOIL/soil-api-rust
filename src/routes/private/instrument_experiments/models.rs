use super::db::Model;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use crudcrate::{CRUDResource, ToCreateModel, ToUpdateModel, traits::MergeIntoActiveModel};
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, Condition, DatabaseConnection, DbErr, EntityTrait,
    Order, QueryOrder, QuerySelect, TransactionTrait, entity::prelude::*,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

pub const UNSUPPORTED_FILE: &str = "unsupported_file: ";

#[derive(ToSchema, Serialize, Deserialize, ToCreateModel, ToUpdateModel)]
#[active_model = "super::db::ActiveModel"]
pub struct InstrumentExperiment {
    #[crudcrate(update_model = false, create_model = false, on_create = Uuid::new_v4())]
    pub id: Uuid,
    #[crudcrate(update_model = false, create_model = false, on_update = chrono::Utc::now(), on_create = chrono::Utc::now())]
    pub last_updated: DateTime<Utc>,
    pub name: Option<String>,
    pub date: Option<DateTime<Utc>>,
    pub description: Option<String>,
    pub filename: Option<String>,
    pub device_filename: Option<String>,
    pub data_source: Option<String>,
    pub instrument_model: Option<String>,
    pub init_e: Option<f64>,
    pub sample_interval: Option<f64>,
    pub run_time: Option<f64>,
    pub quiet_time: Option<f64>,
    pub sensitivity: Option<f64>,
    pub samples: Option<i32>,
    pub project_id: Option<Uuid>,
    #[crudcrate(update_model = false, create_model = false)]
    pub channels: Vec<super::channels::models::InstrumentExperimentChannel>,
    #[crudcrate(non_db_attr = true, default = 0)]
    pub channel_qty_filled: usize,
    #[crudcrate(non_db_attr = true, default = None)]
    pub data_base64: Option<String>,
}

impl From<Model> for InstrumentExperiment {
    fn from(model: Model) -> Self {
        Self {
            name: model.name,
            date: model.date,
            description: model.description,
            filename: model.filename,
            device_filename: model.device_filename,
            data_source: model.data_source,
            instrument_model: model.instrument_model,
            init_e: model.init_e,
            sample_interval: model.sample_interval,
            run_time: model.run_time,
            quiet_time: model.quiet_time,
            sensitivity: model.sensitivity,
            samples: model.samples,
            id: model.id,
            last_updated: model.last_updated,
            project_id: model.project_id,
            channels: vec![],
            channel_qty_filled: 0,
            data_base64: None,
        }
    }
}

#[async_trait]
impl CRUDResource for InstrumentExperiment {
    type EntityType = super::db::Entity;
    type ColumnType = super::db::Column;
    type ActiveModelType = super::db::ActiveModel;
    type CreateModel = InstrumentExperimentCreate;
    type UpdateModel = InstrumentExperimentUpdate;

    const ID_COLUMN: Self::ColumnType = super::db::Column::Id;
    const RESOURCE_NAME_SINGULAR: &'static str = "experiment (lab instrument)";
    const RESOURCE_NAME_PLURAL: &'static str = "experiments (lab instrument)";
    const RESOURCE_DESCRIPTION: &'static str =
        "This represents a specific lab experiment and coincides with the channel data.";

    async fn get_all(
        db: &DatabaseConnection,
        condition: Condition,
        order_column: Self::ColumnType,
        order_direction: Order,
        offset: u64,
        limit: u64,
    ) -> Result<Vec<Self>, DbErr> {
        let objs = Self::EntityType::find()
            .filter(condition)
            .order_by(order_column, order_direction)
            .offset(offset)
            .limit(limit)
            .all(db)
            .await?;

        let mut experiments = Vec::new();
        for experiment in objs {
            let channels = experiment
                .find_related(super::channels::db::Entity)
                .select_only()
                .column(super::channels::db::Column::Id)
                .column(super::channels::db::Column::ChannelName)
                .column(super::channels::db::Column::ExperimentId)
                .column(super::channels::db::Column::IntegralResults)
                .column(super::channels::db::Column::BaselineValues)
                .all(db)
                .await?;

            let mut obj = InstrumentExperiment::from(experiment);
            obj.channels = channels
                .clone()
                .into_iter()
                .map(super::channels::models::InstrumentExperimentChannel::from)
                .collect();
            // Channels filled is the number of channels that have data in baseline_values
            // This (and removing baseline_values) is to limit amount of data
            // sent to the client
            obj.channel_qty_filled = obj
                .channels
                .iter()
                .filter(|c| c.baseline_values.is_some())
                .count();

            for channel in &mut obj.channels {
                channel.baseline_values = Some(serde_json::Value::Array(vec![]));
            }

            experiments.push(obj);
        }

        Ok(experiments)
    }

    async fn get_one(db: &DatabaseConnection, id: Uuid) -> Result<Self, DbErr> {
        let model = Self::EntityType::find()
            .find_with_related(super::channels::db::Entity)
            .filter(Self::ColumnType::Id.eq(id))
            .all(db)
            .await?
            .into_iter()
            .next()
            .ok_or(DbErr::RecordNotFound(format!(
                "{} not found",
                Self::RESOURCE_NAME_SINGULAR
            )))?;

        let (model, channels) = model;
        let mut obj = InstrumentExperiment::from(model);
        obj.channels = channels
            .into_iter()
            .map(super::channels::models::InstrumentExperimentChannel::from)
            .collect();
        Ok(obj)
    }

    async fn create(
        db: &DatabaseConnection,
        create_model: Self::CreateModel,
    ) -> Result<Self, DbErr> {
        let Some(ref data_base64) = create_model.data_base64 else {
            let active_model: Self::ActiveModelType = create_model.into();
            // exec would try to unpack a uuid last_insert_id, which fails on sqlite
            let id = active_model.id.clone().unwrap();
            Self::EntityType::insert(active_model)
                .exec_without_returning(db)
                .await?;
            return Self::get_one(db, id).await;
        };

        let parsed = super::services::process_instrument_data_base64(data_base64)
            .map_err(|e| DbErr::Custom(format!("{UNSUPPORTED_FILE}{e}")))?;

        let mut active_model: Self::ActiveModelType = create_model.clone().into();
        if create_model.date.is_none() {
            active_model.date = ActiveValue::Set(parsed.date);
        }
        if create_model.instrument_model.is_none() {
            active_model.instrument_model = ActiveValue::Set(parsed.instrument_model);
        }
        if create_model.device_filename.is_none() {
            active_model.device_filename = ActiveValue::Set(parsed.device_filename);
        }
        if create_model.data_source.is_none() {
            active_model.data_source = ActiveValue::Set(parsed.data_source);
        }
        if create_model.init_e.is_none() {
            active_model.init_e = ActiveValue::Set(parsed.init_e);
        }
        if create_model.sample_interval.is_none() {
            active_model.sample_interval = ActiveValue::Set(parsed.sample_interval);
        }
        if create_model.run_time.is_none() {
            active_model.run_time = ActiveValue::Set(parsed.run_time);
        }
        if create_model.quiet_time.is_none() {
            active_model.quiet_time = ActiveValue::Set(parsed.quiet_time);
        }
        if create_model.sensitivity.is_none() {
            active_model.sensitivity = ActiveValue::Set(parsed.sensitivity);
        }

        let txn = db.begin().await?;
        let experiment_id = active_model.id.clone().unwrap();
        Self::EntityType::insert(active_model)
            .exec_without_returning(&txn)
            .await?;

        let mut channel_models = Vec::with_capacity(parsed.channel_names.len());
        for (channel_name, raw_values) in
            parsed.channel_names.iter().zip(&parsed.channel_values)
        {
            let time_values = serde_json::to_value(&parsed.time_values)
                .map_err(|e| DbErr::Custom(e.to_string()))?;
            let raw_values =
                serde_json::to_value(raw_values).map_err(|e| DbErr::Custom(e.to_string()))?;
            channel_models.push(super::channels::db::ActiveModel {
                id: ActiveValue::Set(Uuid::new_v4()),
                channel_name: ActiveValue::Set(channel_name.clone()),
                experiment_id: ActiveValue::Set(experiment_id),
                time_values: ActiveValue::Set(Some(time_values)),
                raw_values: ActiveValue::Set(Some(raw_values)),
                baseline_spline: ActiveValue::Set(None),
                baseline_values: ActiveValue::Set(None),
                baseline_chosen_points: ActiveValue::Set(None),
                integral_chosen_pairs: ActiveValue::Set(None),
                integral_results: ActiveValue::Set(None),
            });
        }
        super::channels::db::Entity::insert_many(channel_models)
            .exec_without_returning(&txn)
            .await?;
        txn.commit().await?;

        Self::get_one(db, experiment_id).await
    }

    async fn update(
        db: &DatabaseConnection,
        id: Uuid,
        update_model: Self::UpdateModel,
    ) -> Result<Self, DbErr> {
        let db_obj: super::db::ActiveModel = super::db::Entity::find_by_id(id)
            .one(db)
            .await?
            .ok_or(DbErr::RecordNotFound(format!(
                "{} not found",
                Self::RESOURCE_NAME_SINGULAR
            )))?
            .into();

        let updated_obj: super::db::ActiveModel = update_model.merge_into_activemodel(db_obj);
        let response_obj = updated_obj.update(db).await?;
        let obj = Self::get_one(db, response_obj.id).await?;
        Ok(obj)
    }

    fn sortable_columns() -> Vec<(&'static str, Self::ColumnType)> {
        vec![
            ("id", Self::ColumnType::Id),
            ("name", Self::ColumnType::Name),
            ("last_updated", Self::ColumnType::LastUpdated),
        ]
    }

    fn filterable_columns() -> Vec<(&'static str, Self::ColumnType)> {
        vec![
            ("name", Self::ColumnType::Name),
            ("description", Self::ColumnType::Description),
            ("project_id", Self::ColumnType::ProjectId),
        ]
    }
}
