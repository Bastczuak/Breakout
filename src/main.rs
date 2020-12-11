use amethyst::assets::{AssetStorage, Loader};
use amethyst::audio::output::Output;
use amethyst::audio::{AudioBundle, Source, SourceHandle, WavFormat};
use amethyst::core::ecs::{Builder, Entity, World, WorldExt};
use amethyst::core::math::Vector3;
use amethyst::core::{Transform, TransformBundle};
use amethyst::input::{
  is_close_requested, is_key_down, is_key_up, InputBundle, StringBindings, VirtualKeyCode,
};
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

const VIRTUAL_WIDTH: f32 = 432.;
const VIRTUAL_HEIGHT: f32 = 243.;
const BG_WIDTH: f32 = 302.;
const BG_HEIGHT: f32 = 129.;

struct Sounds {
  paddle_hit_sfx: SourceHandle,
}

#[derive(Default)]
struct Breakout {
  start_ui_text: Option<Entity>,
  high_score_ui_text: Option<Entity>,
  up_key_pressed: bool,
  down_key_pressed: bool,
}

impl SimpleState for Breakout {
  fn on_start(&mut self, data: StateData<'_, GameData<'_, '_>>) {
    let world = data.world;

    init_camera(world);
    init_audio(world);
    world.exec(|mut creator: UiCreator<'_>| {
      creator.create("ui/text.ron", ());
    });
    let background_sprite = load_sprite(
      "textures/background.png",
      "textures/background.ron",
      0,
      world,
    );

    let mut transform = Transform::from(Vector3::new(0., 0., 1.1));
    transform.set_scale(Vector3::new(
      VIRTUAL_WIDTH / (BG_WIDTH - 2.),
      VIRTUAL_HEIGHT / (BG_HEIGHT - 2.),
      1.0,
    ));
    world
      .create_entity()
      .with(background_sprite)
      .with(transform)
      .build();
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
        if let Some(start_ui_text) = self
          .start_ui_text
          .and_then(|entity| ui_text.get_mut(entity))
        {
          start_ui_text.color = [0.4, 1., 1., 1.];
        }
        if let Some(high_score_ui_text) = self
          .high_score_ui_text
          .and_then(|entity| ui_text.get_mut(entity))
        {
          high_score_ui_text.color = [1., 1., 1., 1.];
        }
        play_paddle_hit_sound(&world);
        self.up_key_pressed = true;
      }
      if is_key_down(&event, VirtualKeyCode::Down) && !self.down_key_pressed {
        if let Some(start_ui_text) = self
          .start_ui_text
          .and_then(|entity| ui_text.get_mut(entity))
        {
          start_ui_text.color = [1., 1., 1., 1.];
        }
        if let Some(high_score_ui_text) = self
          .high_score_ui_text
          .and_then(|entity| ui_text.get_mut(entity))
        {
          high_score_ui_text.color = [0.4, 1., 1., 1.];
        }
        play_paddle_hit_sound(&world);
        self.down_key_pressed = true;
      }
      if is_key_up(&event, VirtualKeyCode::Up) {
        self.up_key_pressed = false;
      }
      if is_key_up(&event, VirtualKeyCode::Down) {
        self.down_key_pressed = false;
      }
    }
    Trans::None
  }

  fn update(&mut self, _data: &mut StateData<'_, GameData<'_, '_>>) -> SimpleTrans {
    let StateData { world, .. } = _data;
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

fn init_audio(world: &mut World) {
  let sounds = {
    let loader = world.read_resource::<Loader>();
    Sounds {
      paddle_hit_sfx: load_audio_track_wav(&loader, &world, "sounds/paddle_hit.wav"),
    }
  };
  world.insert(sounds);
}

fn load_audio_track_wav(loader: &Loader, world: &World, file: &str) -> SourceHandle {
  loader.load(file, WavFormat, (), &world.read_resource())
}

fn load_sprite<T>(image: T, ron: T, number: usize, world: &World) -> SpriteRender
where
  T: Into<String>,
{
  let texture_handle = {
    let loader = world.read_resource::<Loader>();
    let texture_storage = world.read_resource::<AssetStorage<Texture>>();
    loader.load(image, ImageFormat::default(), (), &texture_storage)
  };

  let sprite_handle = {
    let loader = world.read_resource::<Loader>();
    let sprite_sheet_store = world.read_resource::<AssetStorage<SpriteSheet>>();
    loader.load(
      ron,
      SpriteSheetFormat(texture_handle),
      (),
      &sprite_sheet_store,
    )
  };

  SpriteRender::new(sprite_handle, number)
}

fn play_paddle_hit_sound(world: &World) {
  let sounds = world.fetch::<Sounds>();
  let output = world.try_fetch::<Output>();
  let storage = world.fetch::<AssetStorage<Source>>();
  if let Some(ref output) = output.as_ref() {
    if let Some(sound) = storage.get(&sounds.paddle_hit_sfx) {
      output.play_once(sound, 0.15);
    }
  }
}

fn main() -> amethyst::Result<()> {
  amethyst::start_logger(Default::default());

  let app_root = application_root_dir()?;
  let display_conf_path = app_root.join("config/display.ron");
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
    .with_bundle(InputBundle::<StringBindings>::new())?
    .with_bundle(UiBundle::<StringBindings>::new())?
    .with_bundle(AudioBundle::default())?;
  let mut game = Application::new(asset_dir, Breakout::default(), game_data)?;

  game.run();
  Ok(())
}
