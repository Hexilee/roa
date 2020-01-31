extern crate proc_macro;
mod dynamic_path;

//use proc_macro::TokenStream;
//use proc_macro_hack::proc_macro_hack;
//use syn::{parse_macro_input, LitStr};
//
//#[proc_macro_hack]
//pub fn dynamic_path(input: TokenStream) -> TokenStream {
//    match dynamic_path::parse(parse_macro_input!(input as LitStr)) {
//        Ok(tokens) => tokens.into(),
//        Err(err) => err.to_compile_error().into(),
//    }
//}
