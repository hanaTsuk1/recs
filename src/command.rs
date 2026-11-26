use crate::{
    Bundle, Entity, World,
    error::{RecsError, RecsResult},
    system::SystemParam,
    world::WorldCell,
};

pub struct Commands<'a> {
    queue: &'a mut CommandQueue,
}

impl<'a> Commands<'a> {
    pub fn spawn<B: Bundle>(&mut self, bundle: B) {
        self.queue.push(SpawnCommand { bundle });
    }

    pub fn despawn(&mut self, entity: Entity) {
        self.queue.push(DespawnCommand(entity));
    }
}

unsafe impl SystemParam for Commands<'_> {
    type Param<'w> = Commands<'w>;

    type State = CommandQueue;

    unsafe fn get_param<'w>(_world: &WorldCell<'w>, state: &'w mut Self::State) -> Self::Param<'w> {
        Commands { queue: state }
    }

    fn init_access(_access: &mut crate::access::Access) {}

    fn init_state() -> Self::State {
        CommandQueue::default()
    }

    fn apply(world: &mut World, state: &mut Self::State) -> Vec<RecsError> {
        state.apply(world)
    }
}

pub trait Command: Send + 'static {
    fn apply(self: Box<Self>, world: &mut World) -> RecsResult<()>;
}

#[derive(Default)]
pub struct CommandQueue {
    commands: Vec<Box<dyn Command>>,
}

impl CommandQueue {
    pub fn push<C: Command>(&mut self, command: C) {
        self.commands.push(Box::new(command));
    }

    pub fn apply(&mut self, world: &mut World) -> Vec<RecsError> {
        let commands = std::mem::take(&mut self.commands);
        let mut errors = Vec::new();

        for command in commands {
            if let Err(error) = command.apply(world) {
                errors.push(error);
            }
        }

        errors
    }
}

struct DespawnCommand(Entity);

impl Command for DespawnCommand {
    fn apply(self: Box<Self>, world: &mut World) -> RecsResult<()> {
        world.despawn(self.0)
    }
}
struct SpawnCommand<B: Bundle> {
    bundle: B,
}

impl<B: Bundle> Command for SpawnCommand<B> {
    fn apply(self: Box<Self>, world: &mut World) -> RecsResult<()> {
        world.spawn(self.bundle);
        Ok(())
    }
}
