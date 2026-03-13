# Kitty config settings reference

Extracted from the uploaded default kitty.conf. For settings with a fixed set of values, the allowed values are listed. For open-ended settings, the expected value format is shown instead.

Total unique config keys/directives found: **211**

## Fonts

| Setting | Default in sample | Possible values / format |
|---|---|---|
| `font_family` | `monospace` | free-form value |
| `bold_font` | `auto` | free-form value |
| `italic_font` | `auto` | free-form value |
| `bold_italic_font` | `auto` | free-form value |
| `font_size` | `11.0` | number or size value |
| `force_ltr` | `no` | yes, no |
| `symbol_map` | `—` | repeatable: symbol_map <codepoints> <font family> |
| `narrow_symbols` | `—` | repeatable: narrow_symbols <codepoints> [cells] |
| `disable_ligatures` | `never` | never, cursor, always |
| `font_features` | `—` | font_features <PostScriptName\|none> <feature...> |
| `modify_font` | `—` | modify_font <field> <value> |
| `box_drawing_scale` | `0.001, 1, 1.5, 2` | four numbers: thin, normal, thick, very_thick |
| `undercurl_style` | `thin-sparse` | thin-sparse, thin-dense, thick-sparse, thick-dense |
| `underline_exclusion` | `1` | number |
| `text_composition_strategy` | `platform` | platform, legacy, or "<gamma> [contrast%]" |
| `text_fg_override_threshold` | `0` | 0, "<ratio> ratio", or "<percent>%" |

## Text cursor customization

| Setting | Default in sample | Possible values / format |
|---|---|---|
| `cursor` | `#cccccc` | color or none |
| `cursor_text_color` | `#111111` | color or background |
| `cursor_shape` | `block` | block, beam, underline |
| `cursor_shape_unfocused` | `hollow` | block, beam, underline, hollow, unchanged |
| `cursor_beam_thickness` | `1.5` | number or size value |
| `cursor_underline_thickness` | `2.0` | number or size value |
| `cursor_blink_interval` | `-1` | seconds, 0, negative, optionally easing |
| `cursor_stop_blinking_after` | `15.0` | number |
| `cursor_trail` | `0` | number |
| `cursor_trail_decay` | `0.1 0.4` | two positive floats: <fast> <slow> |
| `cursor_trail_start_threshold` | `2` | number |
| `cursor_trail_color` | `none` | none or color |

## Scrollback

| Setting | Default in sample | Possible values / format |
|---|---|---|
| `scrollback_lines` | `2000` | number |
| `scrollbar` | `scrolled` | scrolled, hovered, scrolled-and-hovered, always, never |
| `scrollbar_interactive` | `yes` | yes, no |
| `scrollbar_jump_on_click` | `yes` | yes, no |
| `scrollbar_width` | `0.5` | number or size value |
| `scrollbar_hover_width` | `1` | number or size value |
| `scrollbar_handle_opacity` | `0.5` | float |
| `scrollbar_radius` | `0.3` | number or size value |
| `scrollbar_gap` | `0.1` | number or size value |
| `scrollbar_min_handle_height` | `1` | number or size value |
| `scrollbar_hitbox_expansion` | `0.25` | number |
| `scrollbar_track_opacity` | `0` | float |
| `scrollbar_track_hover_opacity` | `0.1` | float |
| `scrollbar_handle_color` | `foreground` | color value (hex, named color, or special keyword where supported) |
| `scrollbar_track_color` | `foreground` | color value (hex, named color, or special keyword where supported) |
| `scrollback_pager` | `less --chop-long-lines --RAW-CONTROL-CHARS +INPUT_LINE_NUMBER` | program command line |
| `scrollback_pager_history_size` | `0` | number or size value |
| `scrollback_fill_enlarged_window` | `no` | yes, no |
| `wheel_scroll_multiplier` | `5.0` | number |
| `wheel_scroll_min_lines` | `1` | number |
| `touch_scroll_multiplier` | `1.0` | number |

## Mouse

| Setting | Default in sample | Possible values / format |
|---|---|---|
| `mouse_hide_wait` | `3.0` | <hide> or <hide> <unhide-wait> <threshold> <scroll-unhide> |
| `url_color` | `#0087bd` | color value (hex, named color, or special keyword where supported) |
| `url_style` | `curly` | none, straight, double, curly, dotted, dashed |
| `open_url_with` | `default` | default or program command |
| `url_prefixes` | `file ftp ftps gemini git gopher http https irc ircs kitty mailto news sftp ssh` | space-separated URL schemes |
| `detect_urls` | `yes` | yes, no |
| `url_excluded_characters` | `—` | string of excluded characters / escapes |
| `show_hyperlink_targets` | `no` | yes, no |
| `underline_hyperlinks` | `hover` | hover, always, never |
| `copy_on_select` | `no` | no, clipboard, or buffer name |
| `clear_selection_on_clipboard_loss` | `no` | yes, no |
| `paste_actions` | `quote-urls-at-prompt,confirm` | comma-separated: quote-urls-at-prompt, replace-dangerous-control-codes, replace-newline, confirm, confirm-if-large, filter, no-op |
| `strip_trailing_spaces` | `never` | never, smart, always |
| `select_by_word_characters` | `@-./_~?&=%+#` | character set string |
| `select_by_word_characters_forward` | `—` | character set string |
| `click_interval` | `-1.0` | number or size value |
| `focus_follows_mouse` | `no` | yes, no |
| `pointer_shape_when_grabbed` | `arrow` | pointer shape name |
| `default_pointer_shape` | `beam` | pointer shape name |
| `pointer_shape_when_dragging` | `beam crosshair` | <shape> [rectangular-shape] |

## Mouse actions

| Setting | Default in sample | Possible values / format |
|---|---|---|
| `clear_all_mouse_actions` | `no` | yes, no |
| `mouse_map` | `left click ungrabbed mouse_handle_click selection link prompt` | mouse_map <button> <event-type> <modes> <action...> |

## Performance tuning

| Setting | Default in sample | Possible values / format |
|---|---|---|
| `repaint_delay` | `10` | number or size value |
| `input_delay` | `3` | number or size value |
| `sync_to_monitor` | `yes` | yes, no |

## Terminal bell

| Setting | Default in sample | Possible values / format |
|---|---|---|
| `enable_audio_bell` | `yes` | yes, no |
| `visual_bell_duration` | `0.0` | number |
| `visual_bell_color` | `none` | none or color |
| `window_alert_on_bell` | `yes` | yes, no |
| `bell_on_tab` | `"🔔 "` | string / symbol / yes-no compatibility values |
| `command_on_bell` | `none` | none or program command |
| `bell_path` | `none` | none or sound file path |
| `linux_bell_theme` | `__custom` | theme name |

## Window layout

| Setting | Default in sample | Possible values / format |
|---|---|---|
| `remember_window_size` | `yes` | yes, no |
| `initial_window_width` | `640` | number or size value |
| `initial_window_height` | `400` | number or size value |
| `remember_window_position` | `no` | yes, no |
| `enabled_layouts` | `*` | *, all, or comma-separated layout names |
| `window_resize_step_cells` | `2` | number or size value |
| `window_resize_step_lines` | `2` | number or size value |
| `window_border_width` | `0.5pt` | size with px or pt |
| `draw_minimal_borders` | `yes` | yes, no |
| `draw_window_borders_for_single_window` | `no` | yes, no |
| `window_margin_width` | `0` | 1, 2, 3, or 4 pt values |
| `single_window_margin_width` | `-1` | negative or 1, 2, 3, or 4 pt values |
| `window_padding_width` | `0` | 1, 2, 3, or 4 pt values |
| `single_window_padding_width` | `-1` | negative or 1, 2, 3, or 4 pt values |
| `placement_strategy` | `center` | top-left, top, top-right, left, center, right, bottom-left, bottom, bottom-right |
| `active_border_color` | `#00ff00` | color or none |
| `inactive_border_color` | `#cccccc` | color |
| `bell_border_color` | `#ff5a00` | color |
| `inactive_text_alpha` | `1.0` | float 0..1 |
| `hide_window_decorations` | `no` | no, yes, titlebar-only, titlebar-and-corners |
| `window_logo_path` | `none` | path or none |
| `window_logo_position` | `bottom-right` | top-left, top, top-right, left, center, right, bottom-left, bottom, bottom-right |
| `window_logo_alpha` | `0.5` | float |
| `window_logo_scale` | `0` | 0, one percentage, or two percentages |
| `resize_debounce_time` | `0.1 0.5` | one or two numbers |
| `resize_in_steps` | `no` | yes, no |
| `visual_window_select_characters` | `1234567890ABCDEFGHIJKLMNOPQRSTUVWXYZ` | unique characters from 0-9A-Z-=[];',./\\` |
| `confirm_os_window_close` | `-1` | integer, optionally "count-background" |

## Tab bar

| Setting | Default in sample | Possible values / format |
|---|---|---|
| `tab_bar_edge` | `bottom` | top, bottom |
| `tab_bar_margin_width` | `0.0` | number or size value |
| `tab_bar_margin_height` | `0.0 0.0` | two pt values |
| `tab_bar_style` | `fade` | fade, slant, separator, powerline, custom, hidden |
| `tab_bar_filter` | `—` | free-form value |
| `tab_bar_align` | `left` | left, center, right |
| `tab_bar_min_tabs` | `2` | number |
| `tab_switch_strategy` | `previous` | previous, left, right, last |
| `tab_fade` | `0.25 0.5 0.75 1` | list of alpha values |
| `tab_separator` | `" ┇"` | string |
| `tab_powerline_style` | `angled` | angled, slanted, round |
| `tab_activity_symbol` | `none` | string or none |
| `tab_title_max_length` | `0` | number |
| `tab_title_template` | `"{fmt.fg.red}{bell_symbol}{activity_symbol}{fmt.fg.tab}{tab.last_focused_progress_percent}{title}"` | template string |
| `active_tab_title_template` | `none` | template string or none |
| `active_tab_foreground` | `#000` | free-form value |
| `active_tab_background` | `#eee` | free-form value |
| `active_tab_font_style` | `bold-italic` | normal, bold, italic, bold-italic |
| `inactive_tab_foreground` | `#444` | free-form value |
| `inactive_tab_background` | `#999` | free-form value |
| `inactive_tab_font_style` | `normal` | normal, bold, italic, bold-italic |
| `tab_bar_background` | `none` | color or none |
| `tab_bar_margin_color` | `none` | color or none |

## Color scheme

| Setting | Default in sample | Possible values / format |
|---|---|---|
| `foreground` | `#dddddd` | free-form value |
| `background` | `#000000` | free-form value |
| `background_opacity` | `1.0` | float 0..1 |
| `background_blur` | `0` | integer/number |
| `transparent_background_colors` | `—` | up to 7 entries: color[@opacity] |
| `dynamic_background_opacity` | `no` | yes, no |
| `background_image` | `none` | path or none |
| `background_image_layout` | `tiled` | tiled, mirror-tiled, scaled, clamped, centered, cscaled |
| `background_image_linear` | `no` | yes, no |
| `background_tint` | `0.0` | float 0..1 |
| `background_tint_gaps` | `1.0` | float |
| `dim_opacity` | `0.4` | float 0..1 |
| `selection_foreground` | `#000000` | color or none |
| `selection_background` | `#fffacd` | color or none |

## The color table

| Setting | Default in sample | Possible values / format |
|---|---|---|
| `color0` | `#000000` | color value (hex, named color, or special keyword where supported) |
| `color8` | `#767676` | color value (hex, named color, or special keyword where supported) |
| `color1` | `#cc0403` | color value (hex, named color, or special keyword where supported) |
| `color9` | `#f2201f` | color value (hex, named color, or special keyword where supported) |
| `color2` | `#19cb00` | color value (hex, named color, or special keyword where supported) |
| `color10` | `#23fd00` | color value (hex, named color, or special keyword where supported) |
| `color3` | `#cecb00` | color value (hex, named color, or special keyword where supported) |
| `color11` | `#fffd00` | color value (hex, named color, or special keyword where supported) |
| `color4` | `#0d73cc` | color value (hex, named color, or special keyword where supported) |
| `color12` | `#1a8fff` | color value (hex, named color, or special keyword where supported) |
| `color5` | `#cb1ed1` | color value (hex, named color, or special keyword where supported) |
| `color13` | `#fd28ff` | color value (hex, named color, or special keyword where supported) |
| `color6` | `#0dcdcd` | color value (hex, named color, or special keyword where supported) |
| `color14` | `#14ffff` | color value (hex, named color, or special keyword where supported) |
| `color7` | `#dddddd` | color value (hex, named color, or special keyword where supported) |
| `color15` | `#ffffff` | color value (hex, named color, or special keyword where supported) |
| `mark1_foreground` | `black` | color |
| `mark1_background` | `#98d3cb` | color |
| `mark2_foreground` | `black` | color |
| `mark2_background` | `#f2dcd3` | color |
| `mark3_foreground` | `black` | color |
| `mark3_background` | `#f274bc` | color |

## Advanced

| Setting | Default in sample | Possible values / format |
|---|---|---|
| `shell` | `.` | . or shell command |
| `editor` | `.` | . or editor command |
| `close_on_child_death` | `no` | yes, no |
| `remote_control_password` | `—` | repeatable: remote_control_password "<password>" [actions...\|checker.py] |
| `allow_remote_control` | `no` | password, socket-only, socket, no, yes |
| `listen_on` | `none` | none or unix:/tcp: socket spec |
| `env` | `—` | repeatable: env NAME=VALUE \| NAME= \| NAME \| read_from_shell=PATTERNS... |
| `filter_notification` | `—` | repeatable boolean expression or all |
| `watcher` | `—` | repeatable path |
| `exe_search_path` | `—` | repeatable path, +path, or -path |
| `update_check_interval` | `24` | number or size value |
| `startup_session` | `none` | path or none |
| `clipboard_control` | `write-clipboard write-primary read-clipboard-ask read-primary-ask` | space-separated actions: write-clipboard, read-clipboard, write-primary, read-primary, read-clipboard-ask, read-primary-ask |
| `clipboard_max_size` | `512` | number or size value |
| `file_transfer_confirmation_bypass` | `—` | password string |
| `allow_hyperlinks` | `yes` | yes, no, ask |
| `shell_integration` | `enabled` | enabled, disabled, or space-separated feature disables: no-rc, no-cursor, no-title, no-cwd, no-prompt-mark, no-complete, no-sudo |
| `allow_cloning` | `ask` | ask, yes, no |
| `clone_source_strategies` | `venv,conda,env_var,path` | comma-separated: venv, conda, env_var, path |
| `notify_on_cmd_finish` | `never` | never, unfocused, invisible, always [+ duration] [+ notify\|bell\|notify-bell\|command ...] |
| `term` | `xterm-kitty` | terminal type string |
| `terminfo_type` | `path` | path, direct, none |
| `forward_stdio` | `no` | yes, no |
| `menu_map` | `—` | menu_map <scope> "<menu path>" <action...> |

## OS specific tweaks

| Setting | Default in sample | Possible values / format |
|---|---|---|
| `wayland_titlebar_color` | `system` | system, background, or color |
| `macos_titlebar_color` | `system` | system, light, dark, background, or color |
| `macos_option_as_alt` | `no` | no, left, right, both |
| `macos_hide_from_tasks` | `no` | yes, no |
| `macos_quit_when_last_window_closed` | `no` | yes, no |
| `macos_window_resizable` | `yes` | yes, no |
| `macos_thicken_font` | `0` | number |
| `macos_traditional_fullscreen` | `no` | yes, no |
| `macos_show_window_title_in` | `all` | window, menubar, all, none |
| `macos_menubar_title_max_length` | `0` | number |
| `macos_custom_beam_cursor` | `no` | yes, no |
| `macos_colorspace` | `srgb` | srgb, default, displayp3 |
| `linux_display_server` | `auto` | auto, x11, wayland |
| `wayland_enable_ime` | `yes` | yes, no |

## Keyboard shortcuts

| Setting | Default in sample | Possible values / format |
|---|---|---|
| `kitty_mod` | `ctrl+shift` | modifier chord, e.g. ctrl+shift |
| `clear_all_shortcuts` | `no` | yes, no |
| `action_alias` | `—` | action_alias <name> <expanded action...> |
| `kitten_alias` | `—` | kitten_alias <name> <expanded kitten invocation...> |