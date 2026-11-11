# "Send with Carrova" for Nautilus (nautilus-python). Written by Carrova;
# turning the entry off in Carrova's settings removes this file.
import os
import subprocess

from gi.repository import GObject, Nautilus

LAUNCHER = {{py}}


class CarrovaSend(GObject.GObject, Nautilus.MenuProvider):
    # Nautilus 3 passes (window, files), Nautilus 4 passes (files).
    def get_file_items(self, *args):
        if not os.access(LAUNCHER, os.X_OK):
            return []
        paths = [f.get_location().get_path() for f in args[-1]]
        if not paths or None in paths:  # local files only
            return []
        item = Nautilus.MenuItem(
            name="CarrovaSend::send", label="Send with Carrova", icon="carrova-desktop"
        )
        item.connect("activate", self._send, paths)
        return [item]

    def _send(self, _item, paths):
        subprocess.Popen([LAUNCHER, "--send", *paths], start_new_session=True)
