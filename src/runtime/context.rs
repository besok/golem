use crate::runtime::action::Tick;
use crate::runtime::args::{RtArgs, RtValue};
use crate::runtime::blackboard::{BBRef, BlackBoard};
use crate::runtime::env::{RtEnvRef};
use crate::runtime::forester::flow::REASON;
use crate::runtime::rtree::rnode::RNodeId;
use crate::runtime::trimmer::{TrimmingQueue, TrimmingQueueRef};
use crate::runtime::{RtOk, RtResult, RuntimeError, TickResult};
use crate::tracer::Event::NewState;
use crate::tracer::{Event, Tracer};
use std::collections::{HashMap, VecDeque};
use std::fmt::{Display, Formatter};
use std::sync::Arc;
use std::sync::Mutex;

pub type Timestamp = usize;
pub type TracerRef = Arc<Mutex<Tracer>>;

/// The remote context ref for the remote actions.
/// Since, the context is supposed to help to send
/// the information to the remote action it does not have the actual copy of the blackboard and tracer.
///
/// #Note
/// The port defines the port of the http server
/// that is used to send the information to the remote action(current http server).
pub struct TreeRemoteContextRef {
    pub curr_ts: Timestamp,
    pub port: u16,
    pub env: RtEnvRef,
}

impl TreeRemoteContextRef {
    pub fn new(curr_ts: Timestamp, port: u16, env: RtEnvRef) -> Self {
        Self { curr_ts, port, env }
    }
}

/// The context ref for the tree to help the actions to implement the logic.
/// The context ref is supposed to be used by the actions
/// to get the information about the current state of the tree.
#[derive(Clone)]
pub struct TreeContextRef {
    bb: BBRef,
    tracer: TracerRef,
    curr_ts: Timestamp,
    _trimmer: TrimmingQueueRef,
    env: RtEnvRef,
}

impl From<&mut TreeContext> for TreeContextRef {
    fn from(value: &mut TreeContext) -> Self {
        TreeContextRef::from_ctx(value, Default::default())
    }
}

impl TreeContextRef {
    pub fn from_ctx(ctx: &TreeContext, trimmer: Arc<Mutex<TrimmingQueue>>) -> Self {
        TreeContextRef::new(
            ctx.bb.clone(),
            ctx.tracer.clone(),
            ctx.curr_ts,
            trimmer,
            ctx.rt_env.clone(),
        )
    }
    /// A pointer to tracer struct.
    pub fn tracer(&self) -> TracerRef {
        self.tracer.clone()
    }
    /// create a trace message
    pub fn trace(&self, ev: String) -> RtOk {
        self.tracer.lock()?.trace(self.curr_ts, Event::Custom(ev))
    }
    pub fn trace_ev(&self, ev: Event) -> RtOk {
        self.tracer.lock()?.trace(self.curr_ts, ev)
    }
    /// A pointer to bb struct.
    pub fn bb(&self) -> BBRef {
        self.bb.clone()
    }

    pub fn env(&self) -> RtEnvRef {
        self.env.clone()
    }
    /// A current tick.
    pub fn current_tick(&self) -> Timestamp {
        self.curr_ts
    }
    pub fn new(
        bb: Arc<Mutex<BlackBoard>>,
        tracer: Arc<Mutex<Tracer>>,
        curr_ts: Timestamp,
        _trimmer: Arc<Mutex<TrimmingQueue>>,
        env: RtEnvRef,
    ) -> Self {
        Self {
            bb,
            tracer,
            curr_ts,
            _trimmer,
            env,
        }
    }
}

/// The runtime context.
/// It is used to store the information about the current state of the tree.
pub struct TreeContext {
    /// Storage
    bb: BBRef,
    /// Tracer to save the tracing information.
    tracer: TracerRef,

    /// The call stack
    stack: VecDeque<RNodeId>,

    /// The latest state for the node
    state: HashMap<RNodeId, RNodeState>,

    /// The latest tick for the node
    ts_map: HashMap<RNodeId, Timestamp>,

    /// Current tick
    curr_ts: Timestamp,

    /// the max amount of ticks
    tick_limit: Timestamp,

    /// The runtime environment
    rt_env: RtEnvRef,
}

impl TreeContext {
    pub fn state(&self) -> &HashMap<RNodeId, RNodeState> {
        &self.state
    }

    /// A pointer to bb struct.
    pub fn bb(&mut self) -> Arc<Mutex<BlackBoard>> {
        self.bb.clone()
    }
    pub fn tracer(&mut self) -> Arc<Mutex<Tracer>> {
        self.tracer.clone()
    }
    pub fn new(bb: BBRef, tracer: TracerRef, tick_limit: Timestamp, rt_env: RtEnvRef) -> Self {
        Self {
            bb,
            tracer,
            stack: Default::default(),
            state: Default::default(),
            ts_map: Default::default(),
            curr_ts: 1,
            tick_limit,
            rt_env,
        }
    }
}

impl TreeContext {
    /// Adds a custom record to the tracer.
    /// Preferably to use `Event::Custom(..)` for that
    pub fn trace(&mut self, ev: Event) -> RtOk {
        self.tracer.lock()?.trace(self.curr_ts, ev)
    }

    pub(crate) fn next_tick(&mut self) -> RtOk {
        self.curr_ts += 1;
        self.trace(Event::NextTick)?;
        debug!(target:"root", "tick up the flow to:{}",self.curr_ts);
        if self.tick_limit != 0 && self.curr_ts >= self.tick_limit {
            Err(RuntimeError::Stopped(format!(
                "the limit of ticks are exceeded on {}",
                self.curr_ts
            )))
        } else {
            Ok(())
        }
    }

    pub(crate) fn root_state(&self, root: RNodeId) -> Tick {
        self.state
            .get(&root)
            .ok_or(RuntimeError::uex(format!("the root node {root} is absent")))
            .and_then(RNodeState::to_tick_result)
    }

    pub(crate) fn is_curr_ts(&self, id: &RNodeId) -> bool {
        self.ts_map
            .get(id)
            .map(|e| *e == self.curr_ts)
            .unwrap_or(false)
    }
    pub fn curr_ts(&self) -> Timestamp {
        self.curr_ts
    }

    pub(crate) fn push(&mut self, id: RNodeId) -> RtOk {
        self.tracer.lock()?.right();
        self.stack.push_back(id);
        Ok(())
    }
    pub(crate) fn pop(&mut self) -> RtResult<Option<RNodeId>> {
        let pop_node = self.stack.pop_back();
        self.tracer.lock()?.left();
        Ok(pop_node)
    }
    pub(crate) fn peek(&self) -> RtResult<Option<&RNodeId>> {
        if self.stack.is_empty() {
            Ok(None)
        } else {
            Ok(self.stack.back())
        }
    }

    pub(crate) fn new_state(
        &mut self,
        id: RNodeId,
        state: RNodeState,
    ) -> RtResult<Option<RNodeState>> {
        self.ts_map.insert(id, self.curr_ts);
        self.trace(NewState(id, state.clone()))?;
        Ok(self.state.insert(id, state))
    }
    pub(crate) fn state_last_set(&self, id: &RNodeId) -> RNodeState {
        self.state
            .get(id)
            .cloned()
            .unwrap_or(RNodeState::Ready(RtArgs::default()))
    }
    pub(crate) fn state_in_ts(&self, id: &RNodeId) -> RNodeState {
        let actual_state = self.state_last_set(id);
        if self.is_curr_ts(&id) {
            actual_state
        } else {
            RNodeState::Ready(actual_state.args())
        }
    }
}

/// The current state of the node.
/// RtArgs here represent the arguments that are passed between ticks and used as meta info
#[derive(Clone, Debug)]
pub enum RNodeState {
    Ready(RtArgs),
    Running(RtArgs),
    Success(RtArgs),
    Failure(RtArgs),
}

impl Display for RNodeState {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            RNodeState::Ready(args) => {
                f.write_str(format!("Ready({})", args).as_str())?;
            }
            RNodeState::Running(args) => {
                f.write_str(format!("Running({})", args).as_str())?;
            }
            RNodeState::Success(args) => {
                f.write_str(format!("Success({})", args).as_str())?;
            }
            RNodeState::Failure(args) => {
                f.write_str(format!("Failure({})", args).as_str())?;
            }
        }
        Ok(())
    }
}

impl RNodeState {
    pub fn from(tick_args: RtArgs, res: TickResult) -> RNodeState {
        match res {
            TickResult::Success => RNodeState::Success(tick_args),
            TickResult::Failure(v) => RNodeState::Failure(tick_args.with(REASON, RtValue::str(v))),
            TickResult::Running => RNodeState::Running(tick_args),
        }
    }
    pub fn to_tick_result(&self) -> RtResult<TickResult> {
        match &self {
            RNodeState::Ready(_) => Err(RuntimeError::uex(
                "the ready is the unexpected state for ".to_string(),
            )),
            RNodeState::Running(_) => Ok(TickResult::running()),
            RNodeState::Success(_) => Ok(TickResult::success()),
            RNodeState::Failure(args) => {
                let reason = args
                    .find(REASON.to_string())
                    .and_then(RtValue::as_string)
                    .unwrap_or_default();

                Ok(TickResult::Failure(reason))
            }
        }
    }

    pub fn is_running(&self) -> bool {
        matches!(self, RNodeState::Running { .. })
    }
    pub fn is_ready(&self) -> bool {
        matches!(self, RNodeState::Ready(_))
    }
    pub fn is_finished(&self) -> bool {
        matches!(self, RNodeState::Success(_) | RNodeState::Failure(_))
    }

    pub fn args(&self) -> RtArgs {
        match self {
            RNodeState::Ready(tick_args)
            | RNodeState::Running(tick_args)
            | RNodeState::Failure(tick_args)
            | RNodeState::Success(tick_args) => tick_args.clone(),
        }
    }
}
