mod game_data;

use crate::game_data::{BreakoutGameData, BreakoutGameDataBuilder};
use amethyst::assets::{AssetStorage, Loader, ProgressCounter};
use amethyst::audio::output::Output;
use amethyst::audio::{AudioBundle, Source, SourceHandle, WavFormat};
use amethyst::core::ecs::Entities;
use amethyst::input::{
  is_close_requested, is_key_down, InputBundle, InputEvent, InputHandler, StringBindings, VirtualKeyCode,
};
use amethyst::renderer::sprite::SpriteSheetHandle;
use amethyst::renderer::types::DefaultBackend;
use amethyst::renderer::{
  Camera, ImageFormat, RenderFlat2D, RenderToWindow, RenderingBundle, SpriteRender, SpriteSheet, SpriteSheetFormat,
  Texture,
};
use amethyst::utils::application_root_dir;
use amethyst::{
  core::{math::Vector3, Hidden, Time, Transform, TransformBundle},
  derive::SystemDesc,
  ecs::prelude::{
    Builder, DenseVecStorage, Entity, Join, NullStorage, Read, ReadStorage, ResourceId, System, SystemData, World,
    WorldExt, WriteStorage,
  },
  ecs::Component,
  ui::{RenderUi, UiBundle, UiCreator, UiFinder, UiText},
};
use amethyst::{Application, State, StateData, StateEvent, Trans};
use std::collections::HashMap;

///
/// constants
///

const VIRTUAL_WIDTH: f32 = 432.;
const VIRTUAL_HEIGHT: f32 = 243.;
const BALL_VELOCITY: f32 = 70.;

///
/// macros
///

macro_rules! assign_text_color {
  ($self:ident, $field_name:ident, $ui_text: ident, $color:tt) => {
    if let Some($field_name) = $self.$field_name.and_then(|entity| $ui_text.get_mut(entity)) {
      $field_name.color = $color;
    }
  };
}

///
/// enums
///

#[derive(Copy, Clone, Eq, Hash, PartialEq)]
enum AssetType {
  Background(usize),
  PaddleSmall(usize),
  PaddleMedium(usize),
  Ball(usize),
}

#[derive(Copy, Clone, Eq, Hash, PartialEq)]
enum SoundType {
  PaddleHit,
  Confirm,
  Pause,
  WallHit,
  BrickHit2,
}

#[derive(Copy, Clone, Eq, Hash, PartialEq)]
enum TextSelectedType {
  Start,
  HighScore,
}

impl Default for TextSelectedType {
  fn default() -> Self {
    TextSelectedType::Start
  }
}

///
/// types
///

#[derive(Component, Debug)]
#[storage(DenseVecStorage)]
struct Paddle {
  width: f32,
  height: f32,
}

#[derive(Component, Debug)]
#[storage(DenseVecStorage)]
struct Ball {
  velocity_x: f32,
  velocity_y: f32,
  radius: f32,
}

#[derive(Component, Debug, Default)]
#[storage(NullStorage)]
struct Player;

#[derive(Default)]
struct SpriteSheetMap(HashMap<AssetType, SpriteSheetHandle>);

#[derive(Default)]
struct SoundMap(HashMap<SoundType, SourceHandle>);

#[derive(SystemData)]
struct Sounds<'a> {
  sound_map: Read<'a, SoundMap>,
  storage: Read<'a, AssetStorage<Source>>,
  output: Option<Read<'a, Output>>,
}

/// functions

fn init_camera(world: &mut World) {
  world
    .create_entity()
    .with(Camera::standard_2d(VIRTUAL_WIDTH, VIRTUAL_HEIGHT))
    .with(Transform::from(Vector3::new(
      VIRTUAL_WIDTH / 2.,
      VIRTUAL_HEIGHT / 2.,
      10.,
    )))
    .build();
}

fn load_sprite_sheet_handle(
  world: &World,
  texture_path: &str,
  ron_path: &str,
  progress_counter: &mut ProgressCounter,
) -> SpriteSheetHandle {
  let texture_handle = {
    let loader = world.read_resource::<Loader>();
    let texture_storage = world.read_resource::<AssetStorage<Texture>>();
    loader.load(texture_path, ImageFormat::default(), (), &texture_storage)
  };
  let loader = world.read_resource::<Loader>();
  let sprite_sheet_store = world.read_resource::<AssetStorage<SpriteSheet>>();
  loader.load(
    ron_path,
    SpriteSheetFormat(texture_handle),
    progress_counter,
    &sprite_sheet_store,
  )
}

fn init_assets(world: &mut World, asset_type_list: Vec<AssetType>) -> ProgressCounter {
  let mut sprite_sheet_map = SpriteSheetMap::default();
  let mut progress_counter = ProgressCounter::new();
  for &asset_type in asset_type_list.iter() {
    let (texture_path, ron_path) = match asset_type {
      AssetType::Background(_) => ("textures/background.png", "textures/background.ron"),
      AssetType::PaddleSmall(_) | AssetType::PaddleMedium(_) => ("textures/breakout.png", "textures/breakout.ron"),
      AssetType::Ball(_) => ("textures/breakout.png", "textures/breakout.ron"),
    };
    let sprite_sheet_handle = load_sprite_sheet_handle(world, texture_path, ron_path, &mut progress_counter);
    sprite_sheet_map.0.insert(asset_type, sprite_sheet_handle);
  }
  world.insert(sprite_sheet_map);
  progress_counter
}

fn init_audio(world: &mut World, sound_type_list: Vec<SoundType>) {
  let mut sound_map = SoundMap::default();
  for &sound_type in sound_type_list.iter() {
    let sound_path = match sound_type {
      SoundType::PaddleHit => "sounds/paddle_hit.wav",
      SoundType::Confirm => "sounds/confirm.wav",
      SoundType::Pause => "sounds/pause.wav",
      SoundType::WallHit => "sounds/wall_hit.wav",
      SoundType::BrickHit2 => "sounds/brick-hit-2.wav",
    };
    let source_handle = {
      let loader = world.read_resource::<Loader>();
      loader.load(sound_path, WavFormat, (), &world.read_resource())
    };
    sound_map.0.insert(sound_type, source_handle);
  }
  world.insert(sound_map);
}

fn play_sound_in_state(world: &World, sound_type: SoundType) {
  let sound_map = world.fetch::<SoundMap>();
  let output = world.try_fetch::<Output>();
  let storage = world.fetch::<AssetStorage<Source>>();
  if let Some(ref output) = output.as_ref() {
    if let Some(sound) = sound_map.0.get(&sound_type) {
      if let Some(sound) = storage.get(&sound) {
        output.play_once(sound, 0.15);
      }
    }
  }
}

fn play_sound_in_system(sounds: &Sounds, sound_type: SoundType) {
  if let Some(ref output) = sounds.output.as_ref() {
    if let Some(sound) = sounds.sound_map.0.get(&sound_type) {
      if let Some(sound) = sounds.storage.get(&sound) {
        output.play_once(sound, 0.15);
      }
    }
  }
}

fn point_in_rect(x: f32, y: f32, left: f32, bottom: f32, right: f32, top: f32) -> bool {
  x >= left && x <= right && y >= bottom && y <= top
}

fn get_texture_dimensions(world: &World, sprite_sheet_handle: &SpriteSheetHandle, sprite_pos: usize) -> (f32, f32) {
  let sprite_sheet_store = world.read_resource::<AssetStorage<SpriteSheet>>();
  let spritesheet = sprite_sheet_store
    .get(&sprite_sheet_handle)
    .expect("Couldn't find the handle for the paddle sprite!");
  (
    spritesheet.sprites[sprite_pos].width,
    spritesheet.sprites[sprite_pos].height,
  )
}

///
/// systems
///

#[derive(Default, SystemDesc)]
struct PaddleSystem;

impl<'a> System<'a> for PaddleSystem {
  type SystemData = (
    WriteStorage<'a, Transform>,
    ReadStorage<'a, Paddle>,
    ReadStorage<'a, Player>,
    Read<'a, InputHandler<StringBindings>>,
    Read<'a, Time>,
  );

  fn run(&mut self, (mut transforms, paddles, player, input, time): Self::SystemData) {
    for (transform, paddle, _) in (&mut transforms, &paddles, &player).join() {
      let horizontal = input.axis_value("horizontal").unwrap_or(0.0);

      if horizontal != 0.0 {
        let dx = time.delta_seconds() * 200.0 * horizontal;
        let paddle_x = transform.translation().x;
        transform.set_translation_x(
          (paddle_x + dx)
            .min(VIRTUAL_WIDTH - paddle.width / 2.)
            .max(paddle.width / 2.),
        );
      }
    }
  }
}

#[derive(Default, SystemDesc)]
struct BallSystem;

impl<'a> System<'a> for BallSystem {
  type SystemData = (WriteStorage<'a, Transform>, ReadStorage<'a, Ball>, Read<'a, Time>);

  fn run(&mut self, (mut transforms, balls, time): Self::SystemData) {
    for (transform, ball) in (&mut transforms, &balls).join() {
      transform.prepend_translation_x(ball.velocity_x * time.delta_seconds());
      transform.prepend_translation_y(ball.velocity_y * time.delta_seconds());
    }
  }
}

#[derive(Default, SystemDesc)]
struct CollisionSystem;

impl<'a> System<'a> for CollisionSystem {
  type SystemData = (
    Entities<'a>,
    WriteStorage<'a, Ball>,
    ReadStorage<'a, Paddle>,
    ReadStorage<'a, Transform>,
    ReadStorage<'a, Player>,
    Sounds<'a>,
  );

  fn run(&mut self, (entities, mut balls, paddles, transforms, players, sounds): Self::SystemData) {
    for (ball, transform) in (&mut balls, &transforms).join() {
      let ball_x = transform.translation().x;
      let ball_y = transform.translation().y;

      if (ball_y <= ball.radius && ball.velocity_y < 0.0)
        || (ball_y >= VIRTUAL_HEIGHT - ball.radius && ball.velocity_y > 0.0)
      {
        play_sound_in_system(&sounds, SoundType::WallHit);
        ball.velocity_y = -ball.velocity_y;
      }

      if (ball_x <= ball.radius && ball.velocity_x < 0.0)
        || (ball_x >= VIRTUAL_WIDTH - ball.radius && ball.velocity_x > 0.0)
      {
        play_sound_in_system(&sounds, SoundType::WallHit);
        ball.velocity_x = -ball.velocity_x;
      }

      for (e, paddle, transform) in (&*entities, &paddles, &transforms).join() {
        let paddle_x = transform.translation().x - (paddle.width * 0.5);
        let paddle_y = transform.translation().y - (paddle.height * 0.5);

        if point_in_rect(
          ball_x,
          ball_y,
          paddle_x - ball.radius,
          paddle_y - ball.radius,
          paddle_x + paddle.width + ball.radius,
          paddle_y + paddle.height + ball.radius,
        ) {
          if let Some(_) = players.get(e) {
            if ball.velocity_y < 0.0 {
              play_sound_in_system(&sounds, SoundType::PaddleHit);
              ball.velocity_y = -ball.velocity_y;
            }
          } else {
            entities
              .delete(e)
              .expect("Couldn't delete paddle while colliding with ball!");
            play_sound_in_system(&sounds, SoundType::BrickHit2);
            ball.velocity_y = -ball.velocity_y;
          }
        }
      }
    }
  }
}

///
/// States
///

#[derive(Default)]
struct StartState {
  title_ui_text: Option<Entity>,
  start_ui_text: Option<Entity>,
  high_score_ui_text: Option<Entity>,
  progress_counter: Option<ProgressCounter>,
  text_selected: TextSelectedType,
}

impl<'a, 'b> State<BreakoutGameData<'a, 'b>, StateEvent> for StartState {
  fn on_start(&mut self, data: StateData<'_, BreakoutGameData<'a, 'b>>) {
    let world = data.world;
    world.exec(|mut creator: UiCreator<'_>| {
      creator.create("ui/text.ron", ());
    });

    init_camera(world);
    init_audio(
      world,
      vec![
        SoundType::PaddleHit,
        SoundType::Confirm,
        SoundType::Pause,
        SoundType::WallHit,
        SoundType::BrickHit2,
      ],
    );
    self.progress_counter = Some(init_assets(
      world,
      vec![
        AssetType::Background(0),
        AssetType::PaddleSmall(0),
        AssetType::PaddleMedium(1),
        AssetType::Ball(2),
      ],
    ));
  }

  fn on_stop(&mut self, data: StateData<'_, BreakoutGameData<'a, 'b>>) {
    let world = data.world;
    let mut hiddens = world.write_storage::<Hidden>();

    if let Some(text) = self.title_ui_text {
      hiddens.insert(text, Hidden).expect("Couldn't hide title text!");
    }
    if let Some(text) = self.start_ui_text {
      hiddens.insert(text, Hidden).expect("Couldn't hide start text!");
    }
    if let Some(text) = self.high_score_ui_text {
      hiddens.insert(text, Hidden).expect("Couldn't hide high score text!");
    }
  }

  fn handle_event(
    &mut self,
    data: StateData<'_, BreakoutGameData<'a, 'b>>,
    event: StateEvent<StringBindings>,
  ) -> Trans<BreakoutGameData<'a, 'b>, StateEvent<StringBindings>> {
    let world = data.world;

    if let StateEvent::Window(event) = &event {
      if is_close_requested(&event) || is_key_down(&event, VirtualKeyCode::Escape) {
        return Trans::Quit;
      }
    }

    if let StateEvent::Input(event) = &event {
      if let InputEvent::KeyPressed { key_code, .. } = event {
        match key_code {
          VirtualKeyCode::Up => {
            let mut ui_text = world.write_storage::<UiText>();
            assign_text_color!(self, start_ui_text, ui_text, [0.4, 1., 1., 1.]);
            assign_text_color!(self, high_score_ui_text, ui_text, [1., 1., 1., 1.]);
            play_sound_in_state(&world, SoundType::PaddleHit);
            self.text_selected = TextSelectedType::Start;
          }
          VirtualKeyCode::Down => {
            let mut ui_text = world.write_storage::<UiText>();
            assign_text_color!(self, start_ui_text, ui_text, [1., 1., 1., 1.]);
            assign_text_color!(self, high_score_ui_text, ui_text, [0.4, 1., 1., 1.]);
            play_sound_in_state(&world, SoundType::PaddleHit);
            self.text_selected = TextSelectedType::HighScore;
          }
          VirtualKeyCode::Return => {
            play_sound_in_state(&world, SoundType::Confirm);
            match self.text_selected {
              TextSelectedType::Start => {
                return Trans::Switch(Box::new(PlayState {
                  title_ui_text: self.title_ui_text,
                  debounce_timer: None,
                }));
              }
              TextSelectedType::HighScore => {}
            }
          }
          _ => {}
        }
      }
    }

    Trans::None
  }

  fn update(
    &mut self,
    mut data: StateData<'_, BreakoutGameData<'a, 'b>>,
  ) -> Trans<BreakoutGameData<'a, 'b>, StateEvent<StringBindings>> {
    let world = &mut data.world;

    if self.title_ui_text.is_none() {
      world.exec(|finder: UiFinder| {
        if let Some(entity) = finder.find("title") {
          self.title_ui_text = Some(entity);
        }
      });
    }
    if self.start_ui_text.is_none() {
      world.exec(|finder: UiFinder<'_>| {
        if let Some(entity) = finder.find("start") {
          self.start_ui_text = Some(entity);
        }
      });
    }
    if self.high_score_ui_text.is_none() {
      world.exec(|finder: UiFinder| {
        if let Some(entity) = finder.find("highscore") {
          self.high_score_ui_text = Some(entity);
        }
      });
    }
    if let Some(ref progress_counter) = self.progress_counter {
      if progress_counter.is_complete() {
        let sprite_sheets_map = {
          let sprite_sheet_map = world.read_resource::<SpriteSheetMap>();
          sprite_sheet_map.0.clone()
        };

        for (asset_type, sprite_sheet_handle) in sprite_sheets_map {
          if let AssetType::Background(sprite_pos) = asset_type {
            let (width, height) = {
              let sprite_sheet_store = world.read_resource::<AssetStorage<SpriteSheet>>();
              let spritesheet = sprite_sheet_store
                .get(&sprite_sheet_handle)
                .expect("Couldn't find the handle for the background sprite!");
              (
                spritesheet.sprites[sprite_pos].width,
                spritesheet.sprites[sprite_pos].height,
              )
            };
            let mut transform = Transform::from(Vector3::new(VIRTUAL_WIDTH / 2., VIRTUAL_HEIGHT / 2., 1.1));
            transform.set_scale(Vector3::new(
              VIRTUAL_WIDTH / (width - 2.),
              VIRTUAL_HEIGHT / (height - 2.),
              1.0,
            ));
            world
              .create_entity()
              .with(SpriteRender::new(sprite_sheet_handle.clone(), sprite_pos))
              .with(transform)
              .build();
          }
        }
        self.progress_counter = None;
      }
    }
    data.data.update(&world, true);

    Trans::None
  }
}

#[derive(Default)]
struct PlayState {
  title_ui_text: Option<Entity>,
  debounce_timer: Option<f32>,
}

impl<'a, 'b> State<BreakoutGameData<'a, 'b>, StateEvent> for PlayState {
  fn on_start(&mut self, data: StateData<'_, BreakoutGameData<'a, 'b>>) {
    let StateData { world, .. } = data;

    if let Some(entity) = self.title_ui_text {
      if let Some(title_ui_text) = world.write_storage::<UiText>().get_mut(entity) {
        title_ui_text.text = String::from("PAUSED");
      }
    }

    let sprite_sheets_map = {
      let sprite_sheet_map = world.read_resource::<SpriteSheetMap>();
      sprite_sheet_map.0.clone()
    };

    for (asset_type, sprite_sheet_handle) in sprite_sheets_map {
      match asset_type {
        AssetType::PaddleMedium(sprite_pos) => {
          let (width, height) = get_texture_dimensions(world, &sprite_sheet_handle, sprite_pos);
          world
            .create_entity()
            .with(Paddle { width, height })
            .with(Player)
            .with(SpriteRender::new(sprite_sheet_handle.clone(), sprite_pos))
            .with(Transform::from(Vector3::new(
              VIRTUAL_WIDTH / 2.,
              VIRTUAL_HEIGHT / 2. - VIRTUAL_HEIGHT / 2.0 + 16.0,
              1.2,
            )))
            .build();
        }
        AssetType::Ball(sprite_pos) => {
          let width = {
            let sprite_sheet_store = world.read_resource::<AssetStorage<SpriteSheet>>();
            let spritesheet = sprite_sheet_store
              .get(&sprite_sheet_handle)
              .expect("Couldn't find the handle for the ball sprite!");
            spritesheet.sprites[sprite_pos].width
          };
          world
            .create_entity()
            .with(SpriteRender::new(sprite_sheet_handle.clone(), sprite_pos))
            .with(Transform::from(Vector3::new(
              VIRTUAL_WIDTH / 2.,
              VIRTUAL_HEIGHT / 2.,
              1.3,
            )))
            .with(Ball {
              velocity_x: -BALL_VELOCITY,
              velocity_y: -BALL_VELOCITY,
              radius: width / 2.,
            })
            .build();
        }
        AssetType::PaddleSmall(sprite_pos) => {
          let (width, height) = get_texture_dimensions(world, &sprite_sheet_handle, sprite_pos);
          for x in 0..2 {
            for y in 0..9 {
              world
                .create_entity()
                .with(Paddle { width, height })
                .with(SpriteRender::new(sprite_sheet_handle.clone(), sprite_pos))
                .with(Transform::from(Vector3::new(
                  VIRTUAL_WIDTH / 5.2 + y as f32 * width + y as f32 * 2.,
                  VIRTUAL_HEIGHT / 1.2 + x as f32 * height + x as f32 * 4.,
                  1.2,
                )))
                .build();
            }
          }
        }
        _ => {}
      }
    }
  }

  fn on_pause(&mut self, data: StateData<'_, BreakoutGameData<'a, 'b>>) {
    let StateData { world, .. } = data;
    let mut hiddens = world.write_storage::<Hidden>();

    play_sound_in_state(&world, SoundType::Pause);
    if let Some(entity) = self.title_ui_text {
      hiddens.remove(entity).expect("Couldn't show paused text!");
    }
  }

  fn on_resume(&mut self, data: StateData<'_, BreakoutGameData<'a, 'b>>) {
    let StateData { world, .. } = data;
    let mut hiddens = world.write_storage::<Hidden>();

    self.debounce_timer = Some(0.25);

    play_sound_in_state(&world, SoundType::Pause);
    if let Some(entity) = self.title_ui_text {
      hiddens.insert(entity, Hidden).expect("Couldn't hide paused text!");
    }
  }

  fn handle_event(
    &mut self,
    _data: StateData<'_, BreakoutGameData<'a, 'b>>,
    event: StateEvent<StringBindings>,
  ) -> Trans<BreakoutGameData<'a, 'b>, StateEvent<StringBindings>> {
    if let StateEvent::Window(event) = &event {
      if is_close_requested(&event) || is_key_down(&event, VirtualKeyCode::Escape) {
        return Trans::Quit;
      }
    }

    if let StateEvent::Input(event) = &event {
      if let InputEvent::KeyPressed { key_code, .. } = event {
        if let VirtualKeyCode::Space = key_code {
          if self.debounce_timer.is_none() {
            return Trans::Push(Box::new(PausedState));
          }
        }
      }
    }

    Trans::None
  }

  fn update(
    &mut self,
    data: StateData<'_, BreakoutGameData<'a, 'b>>,
  ) -> Trans<BreakoutGameData<'a, 'b>, StateEvent<StringBindings>> {
    let StateData { world, .. } = data;

    if let Some(mut time) = self.debounce_timer.take() {
      time -= world.fetch::<Time>().delta_seconds();
      if time >= 0.0 {
        self.debounce_timer.replace(time);
      }
    }

    data.data.update(&world, true);

    Trans::None
  }
}

#[derive(Default)]
struct PausedState;

impl<'a, 'b> State<BreakoutGameData<'a, 'b>, StateEvent> for PausedState {
  fn handle_event(
    &mut self,
    _data: StateData<'_, BreakoutGameData<'a, 'b>>,
    event: StateEvent<StringBindings>,
  ) -> Trans<BreakoutGameData<'a, 'b>, StateEvent<StringBindings>> {
    if let StateEvent::Window(event) = &event {
      if is_close_requested(&event) || is_key_down(&event, VirtualKeyCode::Escape) {
        return Trans::Quit;
      }
    }

    if let StateEvent::Input(event) = &event {
      if let InputEvent::KeyPressed { key_code, .. } = event {
        if let VirtualKeyCode::Space = key_code {
          return Trans::Pop;
        }
      }
    }

    Trans::None
  }

  fn update(
    &mut self,
    data: StateData<'_, BreakoutGameData<'a, 'b>>,
  ) -> Trans<BreakoutGameData<'a, 'b>, StateEvent<StringBindings>> {
    let StateData { world, .. } = data;
    data.data.update(&world, false);

    Trans::None
  }
}

///
/// main
///
fn main() -> amethyst::Result<()> {
  amethyst::start_logger(Default::default());

  let app_root = application_root_dir()?;
  let display_conf_path = app_root.join("config/display.ron");
  let bindings_config_path = app_root.join("config/bindings.ron");
  let asset_dir = app_root.join("assets");
  let app_builder = Application::build(asset_dir, StartState::default())?;
  let game_data = BreakoutGameDataBuilder::default()
    .with_base_bundle(TransformBundle::new())
    .with_base_bundle(InputBundle::<StringBindings>::new())
    .with_base_bundle(UiBundle::<StringBindings>::new())
    .with_base_bundle(AudioBundle::default())
    .with_base_bundle(
      RenderingBundle::<DefaultBackend>::new()
        .with_plugin(RenderToWindow::from_config_path(display_conf_path)?.with_clear([0., 0., 0., 1.]))
        .with_plugin(RenderFlat2D::default())
        .with_plugin(RenderUi::default()),
    )
    .with_running_bundle(InputBundle::<StringBindings>::new().with_bindings_from_file(bindings_config_path)?)
    .with_running(PaddleSystem, "paddle_system", &["input_system"])
    .with_running(BallSystem, "ball_system", &["paddle_system"])
    .with_running(CollisionSystem, "collision_system", &["paddle_system", "ball_system"]);

  let mut game = app_builder.build(game_data)?;
  game.run();

  Ok(())
}
