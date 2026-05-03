use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{parse_macro_input, Data, DeriveInput, Field, Fields};

/// Derive macro that generates `StreamCodec::encode` and `StreamCodec::decode`
/// by inspecting field types and `#[codec(bitpack)]` attributes.
#[proc_macro_derive(StreamCodec, attributes(codec))]
pub fn derive_stream_codec(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    impl_stream_codec(ast)
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

// Fields are grouped so consecutive #[codec(bitpack)] bools share one wire byte.
enum FieldGroup<'a> {
    Regular(&'a Field),
    Bitpack(Vec<&'a Field>),
}

fn has_bitpack(field: &Field) -> bool {
    field.attrs.iter().any(|attr| {
        attr.path().is_ident("codec")
            && attr
                .parse_args::<syn::Ident>()
                .map(|id| id == "bitpack")
                .unwrap_or(false)
    })
}

fn group_fields(
    fields: &syn::punctuated::Punctuated<Field, syn::token::Comma>,
) -> Vec<FieldGroup<'_>> {
    let mut groups: Vec<FieldGroup<'_>> = Vec::new();
    for field in fields {
        if has_bitpack(field) {
            match groups.last_mut() {
                Some(FieldGroup::Bitpack(v)) => v.push(field),
                _ => groups.push(FieldGroup::Bitpack(vec![field])),
            }
        } else {
            groups.push(FieldGroup::Regular(field));
        }
    }
    groups
}

fn impl_stream_codec(ast: DeriveInput) -> syn::Result<TokenStream2> {
    let name = &ast.ident;

    let named_fields = match &ast.data {
        Data::Struct(s) => match &s.fields {
            Fields::Named(f) => &f.named,
            _ => return Err(syn::Error::new_spanned(name, "StreamCodec requires named fields")),
        },
        _ => return Err(syn::Error::new_spanned(name, "StreamCodec requires a struct")),
    };

    let all_field_names: Vec<_> = named_fields.iter().map(|f| &f.ident).collect();
    let groups = group_fields(named_fields);

    let mut encode_stmts = Vec::<TokenStream2>::new();
    let mut decode_stmts = Vec::<TokenStream2>::new();
    let mut bitpack_idx = 0usize;

    for group in &groups {
        match group {
            FieldGroup::Regular(field) => {
                let fname = &field.ident;
                let fty = &field.ty;
                encode_stmts.push(quote! {
                    <#fty as crate::protocols::codec::StreamCodec>::encode(&self.#fname, w).await?;
                });
                decode_stmts.push(quote! {
                    let #fname = <#fty as crate::protocols::codec::StreamCodec>::decode(r).await?;
                });
            }

            FieldGroup::Bitpack(bp_fields) => {
                let flags = format_ident!("__flags_{}", bitpack_idx);
                bitpack_idx += 1;

                // encode: fold each bool into its bit position, write one byte
                let pack_exprs = bp_fields.iter().enumerate().map(|(i, f)| {
                    let n = &f.ident;
                    let shift = i as u32;
                    quote! { ((self.#n as u8) << #shift) }
                });
                encode_stmts.push(quote! {
                    let #flags: u8 = 0u8 #(| #pack_exprs)*;
                    w.write_u8(#flags).await?;
                });

                // decode: read one byte, unpack each bit into its field
                decode_stmts.push(quote! { let #flags = r.read_u8().await?; });
                for (i, f) in bp_fields.iter().enumerate() {
                    let n = &f.ident;
                    let bit = i as u32;
                    decode_stmts.push(quote! {
                        let #n = (#flags & (1u8 << #bit)) != 0;
                    });
                }
            }
        }
    }

    Ok(quote! {
        impl crate::protocols::codec::StreamCodec for #name {
            async fn encode<W: tokio::io::AsyncWrite + Unpin>(
                &self,
                w: &mut W,
            ) -> anyhow::Result<()> {
                use tokio::io::AsyncWriteExt;
                #(#encode_stmts)*
                Ok(())
            }

            async fn decode<R: tokio::io::AsyncRead + Unpin>(
                r: &mut R,
            ) -> anyhow::Result<Self> {
                use tokio::io::AsyncReadExt;
                #(#decode_stmts)*
                Ok(Self { #(#all_field_names),* })
            }
        }
    })
}
