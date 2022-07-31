use core::TargetValueRef;
use std::{
    collections::BTreeMap,
    fs::File,
    io::{self, Read},
    iter::IntoIterator,
    path::PathBuf,
};

use ::value::Value;
use clap::Parser;
use value::Secrets;
use vector_common::TimeZone;
use vrl::{diagnostic::Formatter, state, BatchRuntime, Runtime, Target, VrlRuntime};

#[cfg(feature = "repl")]
use super::repl;
use super::Error;

#[derive(Parser, Debug)]
#[clap(name = "VRL", about = "Vector Remap Language CLI")]
pub struct Opts {
    /// The VRL program to execute. The program ".foo = true", for example, sets the event object's
    /// `foo` field to `true`.
    #[clap(name = "PROGRAM")]
    program: Option<String>,

    /// The file containing the event object(s) to handle. JSON events should be one per line..
    #[clap(short, long = "input", parse(from_os_str))]
    input_file: Option<PathBuf>,

    /// The file containing the VRL program to execute. This can be used instead of `PROGRAM`.
    #[clap(short, long = "program", conflicts_with("PROGRAM"), parse(from_os_str))]
    program_file: Option<PathBuf>,

    /// Print the (modified) event object instead of the result of the final expression. Setting
    /// this flag is equivalent to using `.` as the final expression.
    #[clap(short = 'o', long)]
    print_object: bool,

    /// The timezone used to parse dates.
    #[clap(short = 'z', long)]
    timezone: Option<String>,

    /// Should we use the VM to evaluate the VRL
    #[clap(short, long = "runtime", default_value_t)]
    runtime: VrlRuntime,

    // Should the CLI emit warnings
    #[clap(long = "print-warnings")]
    print_warnings: bool,
}

impl Opts {
    fn timezone(&self) -> Result<TimeZone, Error> {
        if let Some(ref tz) = self.timezone {
            TimeZone::parse(tz)
                .ok_or_else(|| Error::Parse(format!("unable to parse timezone: {}", tz)))
        } else {
            Ok(TimeZone::default())
        }
    }

    fn read_program(&self) -> Result<String, Error> {
        match self.program.as_ref() {
            Some(source) => Ok(source.to_owned()),
            None => match self.program_file.as_ref() {
                Some(path) => read(File::open(path)?),
                None => Ok("".to_owned()),
            },
        }
    }

    fn read_into_objects(&self) -> Result<Vec<Value>, Error> {
        let input = match self.input_file.as_ref() {
            Some(path) => read(File::open(path)?),
            None => read(io::stdin()),
        }?;

        match input.as_str() {
            "" => Ok(vec![Value::Object(BTreeMap::default())]),
            _ => input
                .lines()
                .map(|line| Ok(serde_to_vrl(serde_json::from_str(line)?)))
                .collect::<Result<Vec<Value>, Error>>(),
        }
    }

    fn should_open_repl(&self) -> bool {
        self.program.is_none() && self.program_file.is_none()
    }
}

pub fn cmd(opts: &Opts) -> exitcode::ExitCode {
    match run(opts) {
        Ok(_) => exitcode::OK,
        Err(err) => {
            #[allow(clippy::print_stderr)]
            {
                eprintln!("{}", err);
            }
            exitcode::SOFTWARE
        }
    }
}

fn run(opts: &Opts) -> Result<(), Error> {
    let tz = opts.timezone()?;
    // Run the REPL if no program or program file is specified
    if opts.should_open_repl() {
        // If an input file is provided, use that for the REPL objects, otherwise provide a
        // generic default object.
        let repl_objects = if opts.input_file.is_some() {
            opts.read_into_objects()?
        } else {
            default_objects()
        };

        repl(repl_objects, &tz, opts.runtime)
    } else {
        let mut objects = opts.read_into_objects()?;
        let source = opts.read_program()?;
        let (mut program, warnings) =
            vrl::compile(&source, &stdlib::all()).map_err(|diagnostics| {
                Error::Parse(Formatter::new(&source, diagnostics).colored().to_string())
            })?;

        #[allow(clippy::print_stderr)]
        if opts.print_warnings {
            let warnings = Formatter::new(&source, warnings).colored().to_string();
            eprintln!("{warnings}")
        }

        let mut metadata = objects
            .iter()
            .map(|_| Value::Object(BTreeMap::new()))
            .collect::<Vec<_>>();
        let mut secrets = objects.iter().map(|_| Secrets::new()).collect::<Vec<_>>();

        let mut targets = objects
            .iter_mut()
            .zip(metadata.iter_mut())
            .zip(secrets.iter_mut())
            .map(|((value, metadata), secrets)| TargetValueRef {
                value,
                metadata,
                secrets,
            })
            .collect::<Vec<_>>();

        match opts.runtime {
            VrlRuntime::Ast => {
                for mut target in targets {
                    let state = state::Runtime::default();
                    let mut runtime = Runtime::new(state);
                    let result = runtime
                        .resolve(&mut target, &program, &tz)
                        .map(|v| {
                            if opts.print_object {
                                target.value.to_string()
                            } else {
                                v.to_string()
                            }
                        })
                        .map_err(vrl::Terminate::from)
                        .map_err(Error::Runtime);

                    #[allow(clippy::print_stdout)]
                    #[allow(clippy::print_stderr)]
                    match result {
                        Ok(ok) => println!("{}", ok),
                        Err(err) => eprintln!("{}", err),
                    }
                }
            }
            VrlRuntime::AstBatch => {
                let mut runtime = BatchRuntime::new();
                let mut values = vec![Ok(Value::Null); targets.len()];
                let mut states = (0..targets.len())
                    .map(|_| vrl::state::Runtime::default())
                    .collect::<Vec<_>>();
                let mut batch_targets = targets
                    .iter_mut()
                    .map(|target| target as &mut dyn Target)
                    .collect::<Vec<_>>();
                runtime.resolve_batch(
                    &mut values,
                    &mut batch_targets,
                    &mut states,
                    &mut program,
                    tz,
                );

                for (target, result) in targets.into_iter().zip(values) {
                    let result = result
                        .map(|value| {
                            if opts.print_object {
                                target.value.to_string()
                            } else {
                                value.to_string()
                            }
                        })
                        .map_err(vrl::Terminate::from)
                        .map_err(Error::Runtime);

                    #[allow(clippy::print_stdout)]
                    #[allow(clippy::print_stderr)]
                    match result {
                        Ok(ok) => println!("{}", ok),
                        Err(err) => eprintln!("{}", err),
                    }
                }
            }
        }

        Ok(())
    }
}

#[cfg(feature = "repl")]
fn repl(objects: Vec<Value>, timezone: &TimeZone, vrl_runtime: VrlRuntime) -> Result<(), Error> {
    use core::TargetValue;

    let objects = objects
        .into_iter()
        .map(|value| TargetValue {
            value,
            metadata: Value::Object(BTreeMap::new()),
            secrets: Secrets::new(),
        })
        .collect();

    repl::run(objects, timezone, vrl_runtime);
    Ok(())
}

#[cfg(not(feature = "repl"))]
fn repl(_objects: Vec<Value>, _timezone: &TimeZone, _vrl_runtime: VrlRuntime) -> Result<(), Error> {
    Err(Error::ReplFeature)
}

fn serde_to_vrl(value: serde_json::Value) -> Value {
    use serde_json::Value as JsonValue;

    match value {
        JsonValue::Null => ::value::Value::Null,
        JsonValue::Object(v) => v
            .into_iter()
            .map(|(k, v)| (k, serde_to_vrl(v)))
            .collect::<BTreeMap<_, _>>()
            .into(),
        JsonValue::Bool(v) => v.into(),
        JsonValue::Number(v) if v.is_f64() => Value::from_f64_or_zero(v.as_f64().unwrap()),
        JsonValue::Number(v) => v.as_i64().unwrap_or(i64::MAX).into(),
        JsonValue::String(v) => v.into(),
        JsonValue::Array(v) => v.into_iter().map(serde_to_vrl).collect::<Vec<_>>().into(),
    }
}

fn read<R: Read>(mut reader: R) -> Result<String, Error> {
    let mut buffer = String::new();
    reader.read_to_string(&mut buffer)?;

    Ok(buffer)
}

fn default_objects() -> Vec<Value> {
    vec![Value::Object(BTreeMap::new())]
}
