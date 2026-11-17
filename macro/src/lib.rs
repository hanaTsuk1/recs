use proc_macro::TokenStream;
use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::Span;
use quote::quote;
use syn::{DeriveInput, Ident, parse_macro_input, parse_quote};

#[proc_macro_derive(Component)]
pub fn derive_component(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;

    let crate_path = match crate_name("recs") {
        Ok(FoundCrate::Itself) => quote!(crate),
        Ok(FoundCrate::Name(name)) => {
            let ident = Ident::new(&name, Span::call_site());
            quote!(::#ident)
        }
        Err(_) => {
            return syn::Error::new(Span::call_site(), "could not find `recs` in dependencies")
                .to_compile_error()
                .into();
        }
    };

    let mut generics = input.generics.clone();

    generics
        .make_where_clause()
        .predicates
        .push(parse_quote!(Self: 'static));

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    quote! {
        impl #impl_generics #crate_path::Component
            for #name #ty_generics #where_clause
        {}
    }
    .into()
}
