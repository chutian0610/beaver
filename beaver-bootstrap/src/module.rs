use di::ServiceCollection;

/// a module used for di configuration.
///
/// # Example
/// ```
/// use di::ServiceCollection;
/// use beaver_bootstrap::module::Module;
/// use di::*;
///
/// #[injectable]
/// pub struct A;
/// pub struct MyModule;
///
/// impl Module for MyModule {
///     fn configure(&self, binder: &mut ServiceCollection) {
///         binder.add(A::singleton());
///     }
/// }
/// ```
pub trait Module {
    fn configure(&self, binder: &mut ServiceCollection);
}
