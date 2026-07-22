//! `SeaORM` Entity for BDK floating txouts.

use sea_orm::entity::prelude::*;

#[derive(Copy, Clone, Default, Debug, DeriveEntity)]
pub struct Entity;

impl EntityName for Entity {
    fn table_name(&self) -> &'static str {
        "bdk_txout"
    }
}

#[derive(Clone, Debug, PartialEq, DeriveModel, DeriveActiveModel, Eq)]
pub struct Model {
    pub txid: String,
    pub vout: i64,
    pub value: i64,
    pub script: Vec<u8>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveColumn)]
pub enum Column {
    Txid,
    Vout,
    Value,
    Script,
}

#[derive(Copy, Clone, Debug, EnumIter, DerivePrimaryKey)]
pub enum PrimaryKey {
    Txid,
    Vout,
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
            Self::Txid => ColumnType::String(StringLen::None).def(),
            Self::Vout => ColumnType::BigInteger.def(),
            Self::Value => ColumnType::BigInteger.def(),
            Self::Script => ColumnType::Blob.def(),
        }
    }
}

impl RelationTrait for Relation {
    fn def(&self) -> RelationDef {
        panic!("No RelationDef")
    }
}

impl ActiveModelBehavior for ActiveModel {}
