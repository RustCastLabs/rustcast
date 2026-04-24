use block2::RcBlock;
use objc2::runtime::Bool;
use objc2_event_kit::{EKEntityType, EKEventStore};
use objc2_foundation::{NSDate, NSDateFormatter, NSDateFormatterStyle, NSError};

use crate::{
    app::{
        ToApp,
        apps::{App, AppCommand, ICNS_ICON},
    },
    commands::Function,
    utils::icns_data_to_handle,
};

#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    pub event_name: String,
    pub event_url: Option<String>,
    pub time: String,
}

impl ToApp for Event {
    fn to_app(&self) -> App {
        let icons = icns_data_to_handle(ICNS_ICON.to_vec());
        let appcmd = if let Some(url) = &self.event_url {
            AppCommand::Function(Function::OpenRawUrl(url.clone()))
        } else {
            AppCommand::Display
        };
        App::new(self.event_name.clone(), icons, self.time.clone(), appcmd)
    }
}

impl Event {
    pub fn get_events(duration_in_min: u32) -> Vec<Self> {
        unsafe {
            let store = EKEventStore::new();

            let (tx, rx) = std::sync::mpsc::channel::<()>();

            let block = block2::RcBlock::new(move |_: Bool, _: *mut NSError| {
                let _ = tx.send(());
            });

            store.requestFullAccessToEventsWithCompletion(RcBlock::<
                dyn std::ops::Fn(objc2::runtime::Bool, *mut NSError),
            >::as_ptr(&block));

            rx.recv().unwrap();

            let start = NSDate::now();
            let end = NSDate::dateWithTimeIntervalSinceNow((duration_in_min * 60) as f64);

            let calendars = store.calendarsForEntityType(EKEntityType::Event);

            let predicate = store.predicateForEventsWithStartDate_endDate_calendars(
                &start,
                &end,
                Some(&calendars),
            );

            let formatter = NSDateFormatter::new();

            formatter.setDateStyle(NSDateFormatterStyle::MediumStyle);
            formatter.setTimeStyle(NSDateFormatterStyle::ShortStyle);

            store
                .eventsMatchingPredicate(&predicate)
                .iter()
                .map(|x| Event {
                    event_name: x.title().to_string(),
                    event_url: x
                        .URL()
                        .and_then(|url| url.absoluteString().map(|x| x.to_string())),
                    time: formatter.stringFromDate(&x.startDate()).to_string(),
                })
                .collect()
        }
    }
}
