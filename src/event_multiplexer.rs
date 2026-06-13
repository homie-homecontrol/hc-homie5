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
                pub async fn next(&mut self, timeout: std::time::Duration) -> $enum_name {
                    let all_channels_closed = true $(&& self.$field_name.is_closed())*;

                    tokio::select! {
                        $(
                            Some(event) = self.$field_name.recv() => {
                                $enum_name::$variant(event)
                            }
                        )*
                        _ = tokio::time::sleep(timeout), if !all_channels_closed => {
                            log::trace!("Timeout waiting for events");
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

#[cfg(test)]
mod tests {
    use std::time::Duration;

    crate::define_event_multiplexer! {
        #[derive(Debug, PartialEq, Eq)]
        pub enum TestEvent {
            Value(u8) => value_rx,
        }
    }

    #[tokio::test]
    async fn returns_none_when_all_channels_are_closed() {
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        let mut events = TestEventMultiPlexer::new(rx);
        drop(tx);

        let event = tokio::time::timeout(
            Duration::from_millis(50),
            events.next(Duration::from_secs(60)),
        )
        .await
        .expect("closed channels should not wait for the timeout");

        assert_eq!(event, TestEvent::None);
    }

    #[tokio::test]
    async fn drains_buffered_events_before_returning_none() {
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        tx.send(7).await.unwrap();
        drop(tx);

        let mut events = TestEventMultiPlexer::new(rx);

        assert_eq!(
            events.next(Duration::from_secs(60)).await,
            TestEvent::Value(7)
        );

        let event = tokio::time::timeout(
            Duration::from_millis(50),
            events.next(Duration::from_secs(60)),
        )
        .await
        .expect("drained closed channels should not wait for the timeout");

        assert_eq!(event, TestEvent::None);
    }
}
