# Installation

Install with `cargo install --git https://github.com/mklein994/bing-wallpaper`, then add these `systemd` files:

```ini
# ~/.config/systemd/user/bing-wallpaper.service
[Unit]
Description=Update wallpaper metadata from Bing

[Service]
Type=oneshot
ExecStart=%h/.cargo/bin/bing-wallpaper update
```

```ini
# ~/.config/systemd/user/bing-wallpaper.timer
[Unit]
Description=Update image metadata from Bing

[Timer]
OnBootSec=1min
OnUnitActiveSec=1day
Unit=bing-wallpaper.service

[Install]
WantedBy=default.target
```

Then, to automatically change the wallpaper with `feh`, add this to `~/.fehbg` (make sure it's executable):

```sh
#!/bin/sh
feh --no-fehbg --bg-fill "$(~/.cargo/bin/bing-wallpaper)"
```

THen add these `systemd` files:

```ini
# ~/.config/systemd/user/feh-wallpaper.timer
[Unit]
Description=Dynamic wallpaper with feh

[Timer]
OnBootSec=1min
OnUnitActiveSec=10min
Unit=feh-wallpaper.service

[Install]
WantedBy=default.target
```

```ini
# ~/.config/systemd/user/feh-wallpaper.service
[Unit]
Description=Dynamic wallpaper with feh

[Service]
Type=oneshot
ExecStart=%h/.fehbg
```

Refresh and enable them:

```console
$ systemctl --user daemon-reload
$ systemctl --user enable --now bing-wallpaper.timer
$ systemctl --user enable --now feh-wallpaper.timer
```

Customize these as you see fit.
