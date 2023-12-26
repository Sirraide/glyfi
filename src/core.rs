use std::sync::atomic::AtomicBool;
use poise::{CreateReply};
use poise::serenity_prelude::{CacheHttp, CreateMessage, UserId};
use crate::{__glyfi_terminate_bot, Context, Error, Res};
use crate::sql::__glyfi_fini_db;

/// Logging macros. These macros log an informational or error
/// message. Depending on the program stage, the message will
/// be displayed in the terminal or sent to Discord; The `sync`
/// variants always log to the terminal.
#[macro_export]
macro_rules! info {
    ($arg:expr) => { $crate::core::__glyfi_log_internal(&*($arg)).await };
    ($fmt:literal $(,$arg:expr)*) => { $crate::core::__glyfi_log_internal(format!($fmt $(,$arg)*).as_str()).await };
}

#[macro_export]
macro_rules! info_sync {
    ($arg:expr) => { $crate::core::__glyfi_log_internal_sync(&*($arg)) };
    ($fmt:literal $(,$arg:expr)*) => { $crate::core::__glyfi_log_internal_sync(format!($fmt $(,$arg)*).as_str()) };
}

#[macro_export]
macro_rules! err {
    ($arg:expr) => { $crate::core::__glyfi_log_internal_error(&*($arg)).await };
    ($fmt:literal $(,$arg:expr)*) => { $crate::core::__glyfi_log_internal_error(format!($fmt $(,$arg)*).as_str()).await };
}

#[macro_export]
macro_rules! err_sync {
    ($arg:expr) => { $crate::core::__glyfi_log_internal_error_sync(&*($arg)) };
    ($fmt:literal $(,$arg:expr)*) => { $crate::core::__glyfi_log_internal_error_sync(format!($fmt $(,$arg)*).as_str()) };
}

/// Logging.
pub async fn __glyfi_log_internal_error(e: &str) { eprintln!("[Error]: {}", e); }

pub async fn __glyfi_log_internal(e: &str) { eprintln!("[Info]: {}", e); }

pub fn __glyfi_log_internal_error_sync(e: &str) { eprintln!("[Error]: {}", e); }

pub fn __glyfi_log_internal_sync(e: &str) { eprintln!("[Info]: {}", e); }

pub async fn handle_command_error(e: poise::FrameworkError<'_, crate::Data, Error>) {
    // Reply with a message if possible. Otherwise, just log the error.
    let Some(ctx) = e.ctx() else {
        err!(e.to_string());
        return;
    };

    match ctx {
        Context::Application(a) => {
            // Log the entire command string so we have a record of it.
            err!("In invocation of command: {}", a.invocation_string());

            // Get the nested error, if possible.
            let command_error = match e {
                poise::FrameworkError::Command { error, .. } => error.to_string(),
                _ => "".to_string()
            };

            // Log it in case sending it fails.
            err!(command_error);

            // Send a reply to the user, informing them of the error; if that in turn
            // causes an error, there is nothing we can do, so just log and ignore it.
            if let Err(e) = poise::send_application_reply(
                a,
                CreateReply::default()
                    .ephemeral(true)
                    .content(safe_truncate(format!("Error: {}", command_error), 2000)),
            ).await {
                err!(e.to_string());
            }
        }

        // We don’t use prefix commands.
        _ => unreachable!()
    }
}

pub async fn log_command(ctx: Context<'_>) {
    info!(
        "{} invoked command {}",
        ctx.author().name,
        ctx.invocation_string()
    );
}

/// Report an error resulting from a user misusing a command/function.
pub async fn report_user_error(ctx: impl CacheHttp, user: UserId, s: &str) {
    info!("User Error ({}): {}", user, s);

    // Helper for error handling.
    async fn aux(ctx: &impl CacheHttp, user: UserId, s: &str) -> Res {
        // Attempt to DM the user about this.
        let ch = user.create_dm_channel(&ctx).await?;
        ch.send_message(&ctx, CreateMessage::new().content(format!("Error: {}", s))).await?;
        Ok(())
    }

    match aux(&ctx, user, s).await {
        Err(e) => err!("Error trying to notify user about error '{}': {}", s, e),
        _ => {}
    };
}

/// Truncate a string w/o panicking.
pub fn safe_truncate(mut s: String, mut len: usize) -> String {
    if s.len() <= len { return s; }
    if len == 0 {
        s.clear();
        return s;
    }

    while len != 0 {
        if s.is_char_boundary(len) {
            s.truncate(len);
            return s;
        }

        len -= 1;
    }

    unreachable!();
}

/// Terminate the bot gracefully.
pub async fn terminate() {
    // Don’t terminate twice.
    static TERMINATION_LOCK: AtomicBool = AtomicBool::new(false);
    if TERMINATION_LOCK.compare_exchange(
        false,
        true,
        std::sync::atomic::Ordering::SeqCst,
        std::sync::atomic::Ordering::SeqCst,
    ).is_err() { return; }

    // Shutdown asynchronously running code.
    unsafe {
        /*info_sync!("Shutting down worker tasks...");
        if let Some(tsk) = TASK.as_ref() { tsk.abort_handle().abort(); }*/

        info_sync!("Shutting down bot...");
        __glyfi_terminate_bot().await;

        info_sync!("Shutting down DB...");
        __glyfi_fini_db().await;
    }

    // Exit the process.
    info_sync!("Exiting...");
    std::process::exit(0);
}