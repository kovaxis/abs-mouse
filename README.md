Absolute Mouse is a simple but flexible for using an android device as a
graphics-tablet style mouse.
A rectangle in the 'source' display (the android device) is mapped to a rectangle
in the 'target' display (the computer screen).
A configuration file allows for different flexible setups.

Note: For clarity throughout the project the terms `device`, `source` and `server`
are used interchangeably to represent the side of the connection providing events.
Similarly the terms `desktop`, `target` and `client` are used to represent the
side of the connection receiving events.

# Connecting through USB

The app is configured by default to connect through USB.
Follow these steps to connect:

1. Enable USB Debugging in your device.
   
   To do this you must first enable the Developer Options.
   Go to `Settings > About phone` in your device and tap `Build number` 7 times.
   
   Go to `Settings > Developer Options` and enable USB Debugging.
2. Plug in your android device to your computer through a USB cable.
   Unlock your device and allow for USB Debugging to connect, a popup window should
   appear asking for your approval if you hadn't done it before.
3. Open the app in your device first.
4. Run the desktop app in your computer.
   
   If you're getting some `device '(null)' not found` errors that means your
   device is not getting properly recognized by your computer.
   Some devices require installing manufacturer-specific ADB drivers for recognition,
   do a Google search on ADB drivers for your device and install them if needed.


# Connecting through WiFi

The app will by default attempt to connect to an android device plugged in to
a USB port.
To instead connect through a WiFi network follow these steps:

1. Ensure your computer and your android device are connected to the same network.
2. Find out the IP of your android device, usually of the sort `192.168.x.x`.

   For this you can use a Wifi Analyzer app of any sort, you can find one on
   Google Play.
3. Modify the config file (by default `config.txt`).
   Change the line that says `host: "localhost",` to `host: "<device ip>",`.
   Of course replace `<device ip>` for your actual device IP address.
4. Open the app in your device first.
5. Run the desktop app in your computer.
   Beware though, some network and device configurations might block the connection.

When connecting through a network instead USB Debugging does not need to be enabled,
and no cables are needed, but there might be some extra delay to your touches.


# Full configuration file documentation

The configuration file consists of a Rusty Object Notation file representing several
tweaks for the mouse area mapping.
By default 'config.txt' is used as the config file, but a different file can be
used if the program is run through the command line like so:

```
abs-mouse my-config.txt
```

If the configuration file does not exist a default configuration file will be
created for manual editing.

### Source and target

The `source` and `target` fields each represent a rectangular area of the screen,
representing the areas to be mapped.

These rectangles are represented by two _points_ in the screen: a top-left point
(referenced in the config as `min`) and a bottom-right point (referenced in the
config as `max`).
These two points do not have to be strictly ordered.
Reversing them rotates the rectangle by 180°, and playing around with their
X and Y values will reorient the rectangle.

The `source` rectangle is in _normalized coordinates_, where 0.0 represents the
left edge or top edge and 1.0 represents the right edge or bottom edge.
The `target` rectangle is in _pixel coordinates_, with ((0,0)) being the top-left
pixel and your screen resolution being the bottom-right pixel.
The `target` rectangle defaults to your screen resolution.
If your screen resolution changes, you might want to update your config file.

### Clipping

By default the mouse position is clipped to the screen resolution, but you can
restrict the mouse further.
The `clip` field represents a rectangle to clip the mouse into.
The `min` and `max` fields should be strictly ordered for correct results.

### Orientation correction

By default the app does device rotation correction, which is not usually necessary
because the android app does not allow screen rotation.
This configuration is referred to as `correct_device_orientation`, and it ensures
the device orientation matches your desktop screen orientation (usually landscape).
There should be no need to touch this field.

If you specify a portrait rectangle in the `source` field but your desktop target area
has landscape orientation, the `correct_orientation` config will rotate the rectangle
to better match your target area.
Keeping this setting on is recommended.

### Aspect ratio correction

If your source area has a different aspect ratio with your desktop target area
mouse movement in one axis will be distorted.
To correct this you can use the `keep_aspect_ratio` setting which will shrink
the source area in a single axis to match the target aspect ratio.

### Pressure and size ranges

By default the application maps all touches to a mouse move event, but some touches
can be ignored if their pressure or size parameters exit a given range.
To do this set a minimum or maximum bound from `None` to `Some(<bound>)`.
Pressure and size are usually normalized to a `[0, 1]` range by the android device.

For example, if you're using a stylus and want to ignore fat touches which might
come from your hand and not from the stylus, you would set `size_range` to
`[None, Some(0.6)]`.
The best way to get an appropiate threshold number is to test, since pressure and
size varies from device to device.

### Remote host and port

By default the app will attempt to establish a USB connection to an android device
with USB Debugging enabled using the Android Debug Bridge (ADB).
The default `host` and `port` (`localhost` and `8517`) are suited for this purpose.
If connecting through USB it's usually fine to change the `port` config as long
as the port is unused.
If connecting through a network the `port` should be left intact, as otherwise
it will not match the port used by the android app.

See the `Connecting through WiFi` section for how to setup these fields for wireless
usage.

### Android USB port forwarding

Connecting to an android device plugged in through USB is the default connection
mode.
The `android_port` field represents the port used by the android app when connecting
through USB.
This field should not need to be changed, modify it at your own risk.

The `android_attempt_usb_connection` enables automatic `adb` port forwarding.
There should be no reason to disable this setting, even when connecting wirelessly
instead.


# Building from source

To build the desktop app from source you must first download and install
[Rust](https://www.rust-lang.org).
Once you're done simply run `cargo build --release` from the crate root directory
(wherever `Cargo.toml` is located).
