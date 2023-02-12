extern crate proc_macro;

use darling::{ast, util, FromDeriveInput, FromField};
use proc_macro_error::{abort, proc_macro_error};
use quote::quote;
use syn::{spanned::Spanned, Ident, Type};

#[derive(FromDeriveInput)]
#[darling(attributes(table), supports(struct_named))]
struct TableInput {
    pub ident: syn::Ident,
    #[darling(default)]
    pub name: Option<String>,
    pub data: ast::Data<util::Ignored, TableField>,
}

#[derive(FromField)]
#[darling(attributes(table))]
struct TableField {
    pub ident: Option<syn::Ident>,
    pub ty: syn::Type,
    #[darling(default)]
    pub pk: bool,
}

#[proc_macro_derive(Table, attributes(table))]
#[proc_macro_error]
pub fn derive_table(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    let table = TableInput::from_derive_input(&input).unwrap();
    let table_ident = table.ident;
    let table_name = table
        .name
        .unwrap_or_else(|| table_ident.to_string().to_lowercase());

    let fields = match table.data {
        ast::Data::Struct(ref fields) => fields,
        _ => unreachable!("structs only"),
    };

    let pk_ident_ty_pair = extract_pk(&input, fields);
    let pk_idents = pk_ident_ty_pair
        .iter()
        .map(|(ident, _)| ident)
        .collect::<Vec<_>>();
    let pk_args = pk_ident_ty_pair
        .iter()
        .map(|(ident, ty)| quote! {#ident: #ty})
        .collect::<Vec<_>>();

    let where_phrase = match pk_ident_ty_pair.len() {
        0 => unreachable!("primary key not found"),
        1 => format!("WHERE {} = $1", &pk_ident_ty_pair[0].0),
        _ => {
            let where_phrase = pk_ident_ty_pair
                .iter()
                .enumerate()
                .map(|(i, (ident, _))| format!("{} = ${}", ident, i + 1))
                .collect::<Vec<_>>()
                .join(" AND ");
            format!("WHERE {}", where_phrase)
        }
    };

    let get_query = format!("SELECT * FROM {} {}", table_name, where_phrase);

    let get_impl = quote! {
        impl #table_ident {
            /// Retrieves rows with the specified primary key from the database.
            /// Returns an error if the row is not found.
            pub async fn get(db: &sqlx::PgPool, #(#pk_args),*) -> sqlx::Result<Self> {
                sqlx::query_as!(#table_ident, #get_query, #(#pk_idents),*).fetch_one(db).await
            }

            /// Retrieves rows with the specified primary key from the database.
            /// Returns None if the row is not found.
            pub async fn get_optional(db: &sqlx::PgPool, #(#pk_args),*) -> sqlx::Result<Option<Self>> {
                let result = sqlx::query_as!(#table_ident, #get_query, #(#pk_idents),*).fetch_one(db).await;
                match result {
                    Ok(guild) => Ok(Some(guild)),
                    Err(sqlx::Error::RowNotFound) => Ok(None),
                    Err(e) => Err(e),
                }
            }
        }
    };

    let increment = (0..fields.len())
        .map(|i| format!("${}", i + 1))
        .collect::<Vec<_>>()
        .join(", ");

    let create_query = format!(
        "INSERT INTO {} ({}) VALUES ({}) RETURNING *",
        table_name,
        fields.iter().map(|f| f.ident.as_ref().unwrap().to_string()).collect::<Vec<_>>().join(", "),
        increment
    );

    let create_fields = fields.iter().map(|f| {
        let ident = f.ident.as_ref().unwrap();
        quote! { self.#ident }
    }).collect::<Vec<_>>();

    let create_impl = quote! {
        impl #table_ident {
            /// Creates a new row in the database.
            pub async fn create(&self, db: &sqlx::PgPool) -> sqlx::Result<Self> {
                sqlx::query_as!(#table_ident, #create_query, #(#create_fields),*).fetch_one(db).await
            }
        }
    };

    let increment_without_pk = (0..fields.len() - pk_ident_ty_pair.len()).map(|i| format!("${}", i + pk_ident_ty_pair.len() + 1)).collect::<Vec<_>>();

    let update_query = match increment_without_pk.len() {
        1 => format!(
            "UPDATE {} SET {} = {} {} RETURNING *",
            table_name,
            fields.iter().filter(|f| !f.pk).map(|f| f.ident.as_ref().unwrap().to_string()).collect::<Vec<_>>().join(", "),
            increment_without_pk[0],
            where_phrase,
        ),
        _ => format!(
            "UPDATE {} SET ({}) = ({}) {} RETURNING *",
            table_name,
            fields.iter().filter(|f| !f.pk).map(|f| f.ident.as_ref().unwrap().to_string()).collect::<Vec<_>>().join(", "),
            increment_without_pk.join(", "),
            where_phrase,
        ),
    };

    let update_fields = fields.iter().filter(|f| !f.pk).filter_map(|f| {
        match f.pk {
            true => None,
            false => {
                let ident = f.ident.as_ref().unwrap();
                Some(quote! { self.#ident })
            },
        }
    }).collect::<Vec<_>>();

    let pk_ident_args = pk_idents
        .iter()
        .map(|ident| quote! { self.#ident })
        .collect::<Vec<_>>();

    let update_impl = quote! {
        impl #table_ident {
            /// Updates the row in the database.
            pub async fn update(&self, db: &sqlx::PgPool) -> sqlx::Result<Self> {
                sqlx::query_as!(#table_ident, #update_query, #(#pk_ident_args),* , #(#update_fields),*).fetch_one(db).await
            }
        }
    };

    let delele_query = format!(
        "DELETE FROM {} {}",
        table_name, where_phrase
    );

    let delete_impl = quote! {
        impl #table_ident {
            /// Deletes the row from the database.
            pub async fn delete(&self, db: &sqlx::PgPool) -> sqlx::Result<()> {
                sqlx::query!(#delele_query, #(#pk_ident_args),*).execute(db).await?;
                Ok(())
            }
        }
    };

    let list_query = format!("SELECT * FROM {}", table_name);

    let list_impl = quote! {
        impl #table_ident {
            /// Lists all rows in the database.
            pub async fn list(db: &sqlx::PgPool) -> sqlx::Result<Vec<Self>> {
                sqlx::query_as!(#table_ident, #list_query).fetch_all(db).await
            }
        }
    };

    let gen = quote! {
        #get_impl
        #create_impl
        #update_impl
        #delete_impl
        #list_impl
    };
    gen.into()
}

fn extract_pk(input: &syn::DeriveInput, fields: &ast::Fields<TableField>) -> Vec<(Ident, Type)> {
    let pk_fields: Vec<_> = fields.iter().filter(|f| f.pk).collect();
    match pk_fields.len() {
        0 => abort!(input.span(), "Table `{}` has no primary key", input.ident),
        _ => pk_fields
            .iter()
            .map(|pk_field| (pk_field.ident.clone().unwrap(), pk_field.ty.clone()))
            .collect(),
    }
}
