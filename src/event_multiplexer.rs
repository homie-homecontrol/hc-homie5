#[macro_export]
macro_rules! define_event_multiplexer {
    (
        $(#[$enum_attr:meta])*
        pub enum $enum_name:ident {
            $(
                $variant:ident($type:ty) => $field_name:ident
            ),* $(,)?
        }
    ) => {
        // Define the enum with provided attributes
        $(#[$enum_attr])*
        pub enum $enum_name {
            $(
                $variant($type),
            )*
            Timeout,
            None,
        }

        // Dynamically generate the struct name by appending "Multiplexer" to the enum name
        $crate::paste::paste! {
            pub struct [<$enum_name MultiPlexer>] {
                $(
                    pub $field_name: tokio::sync::mpsc::Receiver<$type>,
                )*
            }

            impl [<$enum_name MultiPlexer>] {
                // Constructor to initialize the struct
                #[allow(clippy::too_many_arguments)]
                pub fn new(
                    $(
                        $field_name: tokio::sync::mpsc::Receiver<$type>,
                    )*
                ) -> Self {
                    Self {
                        $(
                            $field_name,
                        )*
                    }
                }

                // The `next` method to fetch the next event
                pub async fn next(&mut self, timeout: u64) -> $enum_name {
                    tokio::select! {
                        $(
                            Some(event) = self.$field_name.recv() => {
                                $enum_name::$variant(event)
                            }
                        )*
                        _ = tokio::time::sleep(std::time::Duration::from_secs(timeout)) => {
                            log::warn!("Timeout waiting for events");
                            $enum_name::Timeout
                        },
                        else => {
                            $enum_name::None
                        }
                    }
                }
            }
        }
    };
}
