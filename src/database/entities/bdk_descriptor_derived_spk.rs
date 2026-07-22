//! `SeaORM` Entity for the BDK derived SPK cache.

use sea_orm::entity::prelude::*;

#[derive(Copy, Clone, Default, Debug, DeriveEntity)]
pub struct Entity;

impl EntityName for Entity {
    fn table_name(&self) -> &'static str {
        "bdk_descriptor_derived_spk"
    }
}

#[derive(Clone, Debug, PartialEq, DeriveModel, DeriveActiveModel, Eq)]
pub struct Model {
    pub descriptor_id: String,
    pub spk_index: i64,
    pub spk: Vec<u8>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveColumn)]
pub enum Column {
    DescriptorId,
    SpkIndex,
    Spk,
}

#[derive(Copy, Clone, Debug, EnumIter, DerivePrimaryKey)]
pub enum PrimaryKey {
    DescriptorId,
    SpkIndex,
}

impl PrimaryKeyTrait for PrimaryKey {
    type ValueType = (String, i64);
    fn auto_increment() -> bool {
        false
    }
}

#[derive(Copy, Clone, Debug, EnumIter)]
pub enum Relation {}

impl ColumnTrait for Column {
    type EntityName = Entity;
    fn def(&self) -> ColumnDef {
        match self {
            Self::DescriptorId => ColumnType::String(StringLen::None).def(),
            Self::SpkIndex => ColumnType::BigInteger.def(),
            Self::Spk => ColumnType::Blob.def(),
        }
    }
}

impl RelationTrait for Relation {
    fn def(&self) -> RelationDef {
        panic!("No RelationDef")
    }
}

impl ActiveModelBehavior for ActiveModel {}
