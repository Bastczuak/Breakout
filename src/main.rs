use amethyst::assets::{AssetStorage, Loader, ProgressCounter};
use amethyst::audio::output::Output;
use amethyst::audio::{AudioBundle, Source, SourceHandle, WavFormat};
use amethyst::core::ecs::{
  Builder, Component, DenseVecStorage, Entity, Join, Read, ReadStorage, System, World, WorldExt,
  WriteStorage,
};
use amethyst::core::math::Vector3;
use amethyst::core::{Hidden, Time, Transform, TransformBundle};
use amethyst::input::{
  is_close_requested, is_key_down, is_key_up, InputBundle, InputHandler, StringBindings,
  VirtualKeyCode,
};
use amethyst::renderer::sprite::SpriteSheetHandle;
use amethyst::renderer::types::DefaultBackend;
use amethyst::renderer::{
  Camera, ImageFormat, RenderFlat2D, RenderToWindow, RenderingBundle, SpriteRender, SpriteSheet,
  SpriteSheetFormat, Texture,
};
use amethyst::ui::{RenderUi, UiBundle, UiCreator, UiFinder, UiText};
use amethyst::utils::application_root_dir;
use amethyst::{
  Application, GameData, GameDataBuilder, SimpleState, SimpleTrans, StateData, StateEvent, Trans,
};
use std::collections::HashMap;

macro_rules! assign_text_color {
  ($self:ident, $field_name:ident, $ui_text: ident, $color:tt) => {
    if let Some($field_name) = $self
      .$field_name
      .and_then(|entity| $ui_text.get_mut(entity))
    {
      $field_name.color = $color;
    }
  };
}

const VIRTUAL_WIDTH: f32 = 432.;
const VIRTUAL_HEIGHT: f32 = 243.;

#[derive(Copy, Clone, Eq, Hash, PartialEq)]
enum AssetType {
  Background(usize),
  PaddleSmall(usize),
  PaddleMedium(usize),
}

#[derive(Copy, Clone, Eq, Hash, PartialEq)]
enum SoundType {
  PaddleHit,
  Confirm,
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

#[derive(Component)]
#[storage(DenseVecStorage)]
struct Paddle {
  width: f32,
}

struct PaddleSystem;

#[derive(Default)]
struct SpriteSheetMap(HashMap<AssetType, SpriteSheetHandle>);
#[derive(Default)]
struct SoundMap(HashMap<SoundType, SourceHandle>);

#[derive(Default)]
struct StartState {
  title_ui_text: Option<Entity>,
  start_ui_text: Option<Entity>,
  high_score_ui_text: Option<Entity>,
  progress_counter: Option<ProgressCounter>,
  up_key_pressed: bool,
  down_key_pressed: bool,
  return_key_pressed: bool,
  text_selected: TextSelectedType,
}

#[derive(Default)]
struct PlayState;

impl<'a> System<'a> for PaddleSystem {
  type SystemData = (
    WriteStorage<'a, Transform>,
    ReadStorage<'a, Paddle>,
    Read<'a, InputHandler<StringBindings>>,
    Read<'a, Time>,
  );

  fn run(&mut self, (mut transforms, paddles, input, time): Self::SystemData) {
    for (transform, paddle) in (&mut transforms, &paddles).join() {
      let horizontal = input.axis_value("horizontal").unwrap_or(0.0);

      if horizontal != 0.0 {
        let dx = time.delta_seconds() * 200.0 * horizontal;
        let paddle_x = transform.translation().x;
        transform.set_translation_x(
          (paddle_x + dx)
            .min(VIRTUAL_WIDTH / 2. - paddle.width / 2.)
            .max(-VIRTUAL_WIDTH / 2. + paddle.width / 2.),
        );
      }
    }
  }
}

impl SimpleState for StartState {
  fn on_start(&mut self, data: StateData<'_, GameData<'_, '_>>) {
    let world = data.world;
    world.exec(|mut creator: UiCreator<'_>| {
      creator.create("ui/text.ron", ());
    });

    init_camera(world);
    init_audio(world, vec![SoundType::PaddleHit, SoundType::Confirm]);
    self.progress_counter = Some(init_assets(
      world,
      vec![
        AssetType::Background(0),
        AssetType::PaddleSmall(0),
        AssetType::PaddleMedium(1),
      ],
    ));
  }

  fn on_stop(&mut self, data: StateData<'_, GameData<'_, '_>>) {
    let world = data.world;
    let mut hiddens = world.write_storage::<Hidden>();

    if let Some(text) = self.title_ui_text {
      hiddens
        .insert(text, Hidden)
        .expect("Couldn't delete title text!");
    }
    if let Some(text) = self.start_ui_text {
      hiddens
        .insert(text, Hidden)
        .expect("Couldn't delete start text!");
    }
    if let Some(text) = self.high_score_ui_text {
      hiddens
        .insert(text, Hidden)
        .expect("Couldn't delete high score text!");
    }
  }

  fn handle_event(
    &mut self,
    _data: StateData<'_, GameData<'_, '_>>,
    event: StateEvent<StringBindings>,
  ) -> SimpleTrans {
    let world = _data.world;
    if let StateEvent::Window(event) = &event {
      if is_close_requested(&event) || is_key_down(&event, VirtualKeyCode::Escape) {
        return Trans::Quit;
      }

      let mut ui_text = world.write_storage::<UiText>();

      if is_key_down(&event, VirtualKeyCode::Up) && !self.up_key_pressed {
        assign_text_color!(self, start_ui_text, ui_text, [0.4, 1., 1., 1.]);
        assign_text_color!(self, high_score_ui_text, ui_text, [1., 1., 1., 1.]);
        play_sound(&world, SoundType::PaddleHit);
        self.text_selected = TextSelectedType::Start;
        self.up_key_pressed = true;
      }
      if is_key_down(&event, VirtualKeyCode::Down) && !self.down_key_pressed {
        assign_text_color!(self, start_ui_text, ui_text, [1., 1., 1., 1.]);
        assign_text_color!(self, high_score_ui_text, ui_text, [0.4, 1., 1., 1.]);
        play_sound(&world, SoundType::PaddleHit);
        self.text_selected = TextSelectedType::HighScore;
        self.down_key_pressed = true;
      }
      if is_key_down(&event, VirtualKeyCode::Return) && !self.return_key_pressed {
        play_sound(&world, SoundType::Confirm);
        self.return_key_pressed = true;
      }

      if is_key_up(&event, VirtualKeyCode::Up) {
        self.up_key_pressed = false;
      }
      if is_key_up(&event, VirtualKeyCode::Down) {
        self.down_key_pressed = false;
      }
      if is_key_up(&event, VirtualKeyCode::Return) {
        self.return_key_pressed = false;
      }

      match self.text_selected {
        TextSelectedType::Start => {
          if self.return_key_pressed {
            return Trans::Switch(Box::new(PlayState));
          }
        }
        TextSelectedType::HighScore => {}
      }
    }
    Trans::None
  }

  fn update(&mut self, data: &mut StateData<'_, GameData<'_, '_>>) -> SimpleTrans {
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
          match asset_type {
            AssetType::Background(sprite_pos) => {
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
              let mut transform = Transform::from(Vector3::new(0., 0., 1.1));
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
            _ => {}
          }
        }
        self.progress_counter = None;
      }
    }

    Trans::None
  }
}

impl SimpleState for PlayState {
  fn on_start(&mut self, data: StateData<'_, GameData<'_, '_>>) {
    let world = data.world;

    let sprite_sheets_map = {
      let sprite_sheet_map = world.read_resource::<SpriteSheetMap>();
      sprite_sheet_map.0.clone()
    };

    for (asset_type, sprite_sheet_handle) in sprite_sheets_map {
      match asset_type {
        AssetType::PaddleSmall(sprite_pos) => {
          let width = {
            let sprite_sheet_store = world.read_resource::<AssetStorage<SpriteSheet>>();
            let spritesheet = sprite_sheet_store
              .get(&sprite_sheet_handle)
              .expect("Couldn't find the handle for the paddle sprite!");
            spritesheet.sprites[sprite_pos].width
          };
          world
            .create_entity()
            .with(Paddle { width })
            .with(SpriteRender::new(sprite_sheet_handle.clone(), sprite_pos))
            .with(Transform::from(Vector3::new(
              0.,
              -VIRTUAL_HEIGHT / 2. + 16.,
              1.2,
            )))
            .build();
        }
        _ => {}
      }
    }
  }

  fn handle_event(
    &mut self,
    _data: StateData<'_, GameData<'_, '_>>,
    event: StateEvent<StringBindings>,
  ) -> SimpleTrans {
    if let StateEvent::Window(event) = &event {
      if is_close_requested(&event) || is_key_down(&event, VirtualKeyCode::Escape) {
        return Trans::Quit;
      }
    }
    Trans::None
  }
}

fn init_camera(world: &mut World) {
  world
    .create_entity()
    .with(Camera::standard_2d(VIRTUAL_WIDTH, VIRTUAL_HEIGHT))
    .with(Transform::from(Vector3::new(0., 0., 10.)))
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
      AssetType::PaddleSmall(_) | AssetType::PaddleMedium(_) => {
        ("textures/breakout.png", "textures/breakout.ron")
      }
    };
    let sprite_sheet_handle =
      load_sprite_sheet_handle(world, texture_path, ron_path, &mut progress_counter);
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
    };
    let source_handle = {
      let loader = world.read_resource::<Loader>();
      loader.load(sound_path, WavFormat, (), &world.read_resource())
    };
    sound_map.0.insert(sound_type, source_handle);
  }
  world.insert(sound_map);
}

fn play_sound(world: &World, sound_type: SoundType) {
  let sound_map = world.read_resource::<SoundMap>();
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

fn main() -> amethyst::Result<()> {
  amethyst::start_logger(Default::default());

  let app_root = application_root_dir()?;
  let display_conf_path = app_root.join("config/display.ron");
  let bindings_config_path = app_root.join("config/bindings.ron");
  let asset_dir = app_root.join("assets");
  let game_data = GameDataBuilder::default()
    .with_bundle(
      RenderingBundle::<DefaultBackend>::new()
        .with_plugin(
          RenderToWindow::from_config_path(display_conf_path)?.with_clear([0., 0., 0., 1.]),
        )
        .with_plugin(RenderFlat2D::default())
        .with_plugin(RenderUi::default()),
    )?
    .with_bundle(TransformBundle::new())?
    .with_bundle(
      InputBundle::<StringBindings>::new().with_bindings_from_file(bindings_config_path)?,
    )?
    .with_bundle(UiBundle::<StringBindings>::new())?
    .with_bundle(AudioBundle::default())?
    .with(PaddleSystem, "paddle_system", &["input_system"]);
  let mut game = Application::new(asset_dir, StartState::default(), game_data)?;

  game.run();
  Ok(())
}
