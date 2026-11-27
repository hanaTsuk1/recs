use proc_macro::TokenStream;
use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::Span;
use quote::quote;
use syn::{DeriveInput, Ident, parse_macro_input, parse_quote};

fn get_crate_path() -> proc_macro2::TokenStream {
    match crate_name("recs") {
        Ok(FoundCrate::Itself) => quote!(crate),
        Ok(FoundCrate::Name(name)) => {
            let ident = Ident::new(&name, Span::call_site());
            quote!(::#ident)
        }
        Err(_) => syn::Error::new(Span::call_site(), "could not find `recs` in dependencies")
            .to_compile_error(),
    }
}

#[proc_macro_derive(Component)]
pub fn derive_component(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;

    let crate_path = get_crate_path();

    let mut generics = input.generics.clone();

    generics
        .make_where_clause()
        .predicates
        .push(parse_quote!(Self: Send + Sync + 'static));

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    quote! {
        impl #impl_generics #crate_path::Component
            for #name #ty_generics #where_clause
        {}
    }
    .into()
}

#[proc_macro_derive(Resource)]
pub fn derive_resource(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;

    let crate_path = get_crate_path();

    let mut generics = input.generics.clone();

    generics
        .make_where_clause()
        .predicates
        .push(parse_quote!(Self: Send + Sync + 'static));

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    quote! {
        impl #impl_generics #crate_path::Component
            for #name #ty_generics #where_clause
        {}
        impl #impl_generics #crate_path::Resource
            for #name #ty_generics #where_clause
        {}
    }
    .into()
}
