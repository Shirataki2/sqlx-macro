extern crate proc_macro;

use darling::{FromDeriveInput, FromField, ast, util};
use proc_macro_error::{proc_macro_error, abort};
use quote::quote;
use syn::{Type, Ident, spanned::Spanned};

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
    let table_name = table.name.unwrap_or_else(|| table_ident.to_string().to_lowercase());

    let fields = match table.data {
        ast::Data::Struct(ref fields) => fields,
        _ => unreachable!("structs only"),
    };

    let (pk_ident, pk_ty) = extract_pk(&input, fields);

    let get_query = format!(
        "SELECT * FROM {} WHERE {} = $1",
        table_name, pk_ident
    );

    let get_impl = quote! {
        impl #table_ident {
            pub async fn get(db: &sqlx::PgPool, #pk_ident: #pk_ty) -> sqlx::Result<Self> {
                sqlx::query_as!(#table_ident, #get_query, #pk_ident).fetch_one(db).await
            }
        }
    };

    let increment = (0..fields.len()).map(|i| format!("${}", i + 1)).collect::<Vec<_>>().join(", ");

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
            pub async fn create(&self, db: &sqlx::PgPool) -> sqlx::Result<Self> {
                sqlx::query_as!(#table_ident, #create_query, #(#create_fields),*).fetch_one(db).await
            }
        }
    };

    let increment_without_pk = (0..fields.len() - 1).map(|i| format!("${}", i + 2)).collect::<Vec<_>>().join(", ");

    let update_query = format!(
        "UPDATE {} SET ({}) = ({}) WHERE {} = $1 RETURNING *",
        table_name,
        fields.iter().filter(|f| !f.pk).map(|f| f.ident.as_ref().unwrap().to_string()).collect::<Vec<_>>().join(", "),
        increment_without_pk,
        pk_ident
    );

    let update_fields = fields.iter().filter(|f| !f.pk).filter_map(|f| {
        match f.pk {
            true => None,
            false => {
                let ident = f.ident.as_ref().unwrap();
                Some(quote! { self.#ident })
            },
        }
    }).collect::<Vec<_>>();

    let update_impl = quote! {
        impl #table_ident {
            pub async fn update(&self, db: &sqlx::PgPool) -> sqlx::Result<Self> {
                sqlx::query_as!(#table_ident, #update_query, self.#pk_ident, #(#update_fields),*).fetch_one(db).await
            }
        }
    };

    let delele_query = format!(
        "DELETE FROM {} WHERE {} = $1",
        table_name, pk_ident
    );

    let delete_impl = quote! {
        impl #table_ident {
            pub async fn delete(&self, db: &sqlx::PgPool) -> sqlx::Result<()> {
                sqlx::query!(#delele_query, self.#pk_ident).execute(db).await?;
                Ok(())
            }
        }
    };

    let gen = quote! {
        #get_impl
        #create_impl
        #update_impl
        #delete_impl
    };
    gen.into()
}

fn extract_pk(input: &syn::DeriveInput, fields: &ast::Fields<TableField>) -> (Ident, Type) {
    let pk_fields: Vec<_> = fields.iter().filter(|f| f.pk).collect();
    match pk_fields.len() {
        0 => abort!(input.span(), "Table `{}` has no primary key", input.ident),
        1 => (pk_fields[0].ident.clone().unwrap(), pk_fields[0].ty.clone()),
        _ => abort!(input.span(), "Table `{}` has multiple primary keys", input.ident),
    }
}