pub use sea_orm_migration::prelude::*;

mod m20250217_105133_first_migration;
mod m20250219_075041_sensorprofile;
mod m20250219_152310_modify_sensordata_moisturecount;
mod m20250226_092944_use_utc_timestamps;
mod m20250226_104416_remove_last_updated_sensordata;
mod m20250227_110710_remove_indexes_on_large_attributes;
mod m20250303_101534_remove_areaid_from_sensors;
mod m20250304_110517_set_unique_transect_name_per_area;
mod m20250307_124139_plot_remove_notnull_gradient;
mod m20250507_101706_add_ispublic_flag_to_areas;
mod m20250508_122136_add_additional_plot_sample_fields;
mod m20250509_133815_add_soil_classification_table;
mod m20250509_152257_add_soil_classification_to_plot_sample;
mod m20250512_075700_add_unique_constraint_to_soil_types;
mod m20250512_114505_remove_notnull_constraints_on_description;
mod m20250512_123505_add_sample_date;
mod m20250515_154112_allow_multiple_sensors_on_sample_soil_profile;
mod m20250526_133243_add_depth_for_sensor_moisture;
mod m20250702_115754_create_enum_soil_type_sensor_profile;
mod m20260226_000000_add_flux_redox_websites;
mod m20260227_000000_precompute_sensor_averages;
mod m20260302_000000_add_6h_continuous_aggregate;
mod m20260729_000000_cascade_delete_experiment_channels;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20250217_105133_first_migration::Migration),
            Box::new(m20250219_075041_sensorprofile::Migration),
            Box::new(m20250219_152310_modify_sensordata_moisturecount::Migration),
            Box::new(m20250226_092944_use_utc_timestamps::Migration),
            Box::new(m20250226_104416_remove_last_updated_sensordata::Migration),
            Box::new(m20250227_110710_remove_indexes_on_large_attributes::Migration),
            Box::new(m20250303_101534_remove_areaid_from_sensors::Migration),
            Box::new(m20250304_110517_set_unique_transect_name_per_area::Migration),
            Box::new(m20250307_124139_plot_remove_notnull_gradient::Migration),
            Box::new(m20250507_101706_add_ispublic_flag_to_areas::Migration),
            Box::new(m20250508_122136_add_additional_plot_sample_fields::Migration),
            Box::new(m20250509_133815_add_soil_classification_table::Migration),
            Box::new(m20250509_152257_add_soil_classification_to_plot_sample::Migration),
            Box::new(m20250512_075700_add_unique_constraint_to_soil_types::Migration),
            Box::new(m20250512_114505_remove_notnull_constraints_on_description::Migration),
            Box::new(m20250512_123505_add_sample_date::Migration),
            Box::new(m20250515_154112_allow_multiple_sensors_on_sample_soil_profile::Migration),
            Box::new(m20250526_133243_add_depth_for_sensor_moisture::Migration),
            Box::new(m20250702_115754_create_enum_soil_type_sensor_profile::Migration),
            Box::new(m20260226_000000_add_flux_redox_websites::Migration),
            Box::new(m20260227_000000_precompute_sensor_averages::Migration),
            Box::new(m20260302_000000_add_6h_continuous_aggregate::Migration),
            Box::new(m20260729_000000_cascade_delete_experiment_channels::Migration),
        ]
    }
}
