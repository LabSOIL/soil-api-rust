// instrument_experiments/channels/models.rs

use async_trait::async_trait;
use crudcrate::{CRUDResource, ToCreateModel, ToUpdateModel, traits::MergeIntoActiveModel};
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, Condition, DatabaseConnection, DbErr, EntityTrait,
    Order, QueryOrder, QuerySelect, entity::prelude::*,
};
use serde::{Deserialize, Serialize};
use serde_json::Value as Json;
use utoipa::ToSchema;
use uuid::Uuid;

use super::db::Model;

/// The API model for an instrument experiment channel.
#[derive(Clone, ToSchema, Serialize, Deserialize, ToCreateModel, ToUpdateModel)]
#[active_model = "super::db::ActiveModel"]
pub struct InstrumentExperimentChannel {
    #[crudcrate(update_model = false, create_model = false, on_create = Uuid::new_v4())]
    pub id: Uuid,
    pub channel_name: String,
    pub experiment_id: Uuid,
    pub baseline_spline: Option<Json>,
    pub time_values: Option<Json>,
    pub raw_values: Option<Json>,
    pub baseline_values: Option<Json>,
    pub baseline_chosen_points: Option<Json>,
    pub integral_chosen_pairs: Option<Json>,
    pub integral_results: Option<Json>,
}

impl From<Model> for InstrumentExperimentChannel {
    fn from(model: Model) -> Self {
        Self {
            id: model.id,
            channel_name: model.channel_name,
            experiment_id: model.experiment_id,
            baseline_spline: model.baseline_spline,
            time_values: model.time_values,
            raw_values: model.raw_values,
            baseline_values: model.baseline_values,
            baseline_chosen_points: model.baseline_chosen_points,
            integral_chosen_pairs: model.integral_chosen_pairs,
            integral_results: model.integral_results,
        }
    }
}

#[async_trait]
impl CRUDResource for InstrumentExperimentChannel {
    type EntityType = super::db::Entity;
    type ColumnType = super::db::Column;
    type ActiveModelType = super::db::ActiveModel;
    type CreateModel = InstrumentExperimentChannelCreate;
    type UpdateModel = InstrumentExperimentChannelUpdate;

    const ID_COLUMN: Self::ColumnType = super::db::Column::Id;
    const RESOURCE_NAME_SINGULAR: &'static str = "channel (instrument experiment)";
    const RESOURCE_NAME_PLURAL: &'static str = "channels (instrument experiment)";
    const RESOURCE_DESCRIPTION: &'static str = "This represents the data for a specific channel during the recording in the lab instrument.";

    async fn get_all(
        db: &DatabaseConnection,
        condition: Condition,
        order_column: Self::ColumnType,
        order_direction: Order,
        offset: u64,
        limit: u64,
    ) -> Result<Vec<Self>, DbErr> {
        let models = Self::EntityType::find()
            .filter(condition)
            .order_by(order_column, order_direction)
            .offset(offset)
            .limit(limit)
            .all(db)
            .await?;
        Ok(models
            .into_iter()
            .map(InstrumentExperimentChannel::from)
            .collect())
    }

    async fn get_one(db: &DatabaseConnection, id: Uuid) -> Result<Self, DbErr> {
        let model = Self::EntityType::find()
            .filter(Self::ColumnType::Id.eq(id))
            .one(db)
            .await?
            .ok_or(DbErr::RecordNotFound(
                "Instrument experiment channel not found".into(),
            ))?;
        Ok(InstrumentExperimentChannel::from(model))
    }

    async fn update(
        db: &DatabaseConnection,
        id: Uuid,
        update_model: Self::UpdateModel,
    ) -> Result<Self, DbErr> {
        // Fetch the original record as an ActiveModel.
        let original_model: super::db::ActiveModel = Self::EntityType::find_by_id(id)
            .one(db)
            .await?
            .ok_or(DbErr::RecordNotFound(
                "Instrument experiment channel not found".into(),
            ))?
            .into();

        // Merge the update payload into the original model.
        let mut active_model = update_model
            .clone()
            .merge_into_activemodel(original_model.clone());

        // --- Process baseline_chosen_points ---
        if let Some(baseline_chosen_points_json) = update_model.baseline_chosen_points {
            let baseline_chosen_points: Vec<serde_json::Value> = baseline_chosen_points_json
                .map_or_else(Vec::new, |json| {
                    serde_json::from_value(json).unwrap_or_default()
                });

            // Extract time values (x) from active_model or fallback to the original.
            let x: Vec<f64> = match &active_model.time_values {
                ActiveValue::Set(Some(json)) | ActiveValue::Unchanged(Some(json)) => {
                    serde_json::from_value(json.clone()).unwrap_or_default()
                }
                _ => match &original_model.time_values {
                    ActiveValue::Set(Some(json)) | ActiveValue::Unchanged(Some(json)) => {
                        serde_json::from_value(json.clone()).unwrap_or_default()
                    }
                    _ => Vec::new(),
                },
            };

            // Extract raw values (y) similarly.
            let y: Vec<f64> = match &active_model.raw_values {
                ActiveValue::Set(Some(json)) | ActiveValue::Unchanged(Some(json)) => {
                    serde_json::from_value(json.clone()).unwrap_or_default()
                }
                _ => match &original_model.raw_values {
                    ActiveValue::Set(Some(json)) | ActiveValue::Unchanged(Some(json)) => {
                        serde_json::from_value(json.clone()).unwrap_or_default()
                    }
                    _ => Vec::new(),
                },
            };

            if baseline_chosen_points.is_empty() {
                // Since the field expects a Json (not an Option), supply a JSON array.
                active_model.baseline_spline = ActiveValue::Set(Some(serde_json::json!([])));
                active_model.baseline_values = ActiveValue::Set(Some(serde_json::json!([])));
            } else {
                // Extract chosen x-values.
                let chosen_points: Vec<f64> = baseline_chosen_points
                    .into_iter()
                    .filter_map(|bp| bp.get("x").and_then(sea_orm::JsonValue::as_f64))
                    .collect();

                let spline = super::tools::calculate_spline(&x, &y, &chosen_points, "linear");

                let filtered_baseline = super::tools::filter_baseline(&y, &spline);

                // Now set the computed values (no extra Some() wrapping).
                active_model.baseline_spline =
                    ActiveValue::Set(Some(serde_json::to_value(&spline).unwrap()));
                active_model.baseline_values =
                    ActiveValue::Set(Some(serde_json::to_value(&filtered_baseline).unwrap()));
            }
        }

        // --- Process integral_chosen_pairs ---
        if let Some(integral_chosen_pairs_json) = update_model.integral_chosen_pairs {
            let baseline_values: Vec<f64> = match &active_model.baseline_values {
                ActiveValue::Set(Some(json)) | ActiveValue::Unchanged(Some(json)) => {
                    serde_json::from_value(json.clone()).unwrap_or_default()
                }
                _ => match &original_model.baseline_values {
                    ActiveValue::Set(Some(json)) | ActiveValue::Unchanged(Some(json)) => {
                        serde_json::from_value(json.clone()).unwrap_or_default()
                    }
                    _ => Vec::new(),
                },
            };

            let time_values: Vec<f64> = match &active_model.time_values {
                ActiveValue::Set(Some(json)) | ActiveValue::Unchanged(Some(json)) => {
                    serde_json::from_value(json.clone()).unwrap_or_default()
                }
                _ => match &original_model.time_values {
                    ActiveValue::Set(Some(json)) | ActiveValue::Unchanged(Some(json)) => {
                        serde_json::from_value(json.clone()).unwrap_or_default()
                    }
                    _ => Vec::new(),
                },
            };

            let integral_chosen_pairs: Vec<serde_json::Value> = integral_chosen_pairs_json
                .map_or_else(Vec::new, |json| {
                    serde_json::from_value(json).unwrap_or_default()
                });

            // Trapezoidal rule as in the lab's MATLAB reference
            // (MER_MEO_Eval_003.m) and the lab-codes workflow default
            let integral_results = super::tools::calculate_integrals_for_pairs(
                &integral_chosen_pairs,
                &baseline_values,
                &time_values,
                "trapz",
            );

            active_model.integral_results =
                ActiveValue::Set(Some(serde_json::to_value(&integral_results).unwrap()));
        }

        // Execute the update in the database.
        let response_obj = active_model.update(db).await?;
        let obj = Self::get_one(db, response_obj.id).await?;

        Ok(obj)
    }

    fn sortable_columns() -> Vec<(&'static str, Self::ColumnType)> {
        vec![
            ("id", super::db::Column::Id),
            ("channel_name", super::db::Column::ChannelName),
        ]
    }

    fn filterable_columns() -> Vec<(&'static str, Self::ColumnType)> {
        vec![("channel_name", super::db::Column::ChannelName)]
    }
}
