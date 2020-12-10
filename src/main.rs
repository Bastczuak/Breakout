use amethyst::audio::AudioBundle;
use amethyst::core::TransformBundle;
use amethyst::input::{InputBundle, StringBindings};
use amethyst::renderer::types::DefaultBackend;
use amethyst::renderer::{RenderFlat2D, RenderToWindow, RenderingBundle};
use amethyst::ui::{RenderUi, UiBundle};
use amethyst::utils::application_root_dir;
use amethyst::{Application, GameDataBuilder, SimpleState};

struct Breakout;

impl SimpleState for Breakout {}

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
  let mut game = Application::new(asset_dir, Breakout, game_data)?;

  game.run();
  Ok(())
}
