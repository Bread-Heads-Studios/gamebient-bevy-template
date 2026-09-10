import subprocess
import unittest
from unittest import mock

from reel import (DEFAULTS, card_args, clip_args, concat_list, fill, fit_size,
                  init_config, is_near_silent, load_config, parse_index,
                  parse_max_volume, resolve_clip, run)

INDEX = """# Video notes index

| Game | Folder | Tour length | Lead clip | Hook (one sentence) | Top payoff not reached by the tour |
|---|---|---|---|---|---|
| Tire Stack | tire-stack | 01:02.48 | `clips/06-delivery-popup.mp4` | Tires drop. | A topple. |
| Pack The Ripper | pack-the-ripper | 01:02.02 | `clips/signature.mp4` (= `06-pack-burst.mp4`) | Rip. | Chase card. |
"""


class IndexTests(unittest.TestCase):
    def test_parse_index_reads_folder_and_first_clip(self):
        self.assertEqual(parse_index(INDEX), [
            {"folder": "tire-stack", "clip": "06-delivery-popup"},
            {"folder": "pack-the-ripper", "clip": "signature"},
        ])

    def test_init_config_applies_defaults(self):
        cfg = init_config(INDEX)
        self.assertEqual(cfg["cta"], ["Play the demo at", "colecovisiongx.com"])
        self.assertEqual(cfg["card_secs"], 1.5)
        self.assertEqual(cfg["end_secs"], 3.0)
        self.assertEqual(cfg["games"][0], {"folder": "tire-stack", "clip": "06-delivery-popup",
                                           "start": 1.0, "length": 4.5})


class ConfigTests(unittest.TestCase):
    def test_load_config_fills_missing_fields(self):
        cfg = load_config('{"games": [{"folder": "gulper", "clip": "06-digest", "length": 3.0}]}')
        self.assertEqual(cfg["card_secs"], DEFAULTS["card_secs"])
        self.assertEqual(cfg["games"][0]["start"], 1.0)
        self.assertEqual(cfg["games"][0]["length"], 3.0)

    def test_resolve_clip_maps_aliases(self):
        beats = ["01-studio-logo", "06-pack-burst", "09-game-over"]
        self.assertEqual(resolve_clip("signature", beats), "06-pack-burst")
        self.assertEqual(resolve_clip("game-over", beats), "09-game-over")
        self.assertEqual(resolve_clip("01-studio-logo", beats), "01-studio-logo")
        with self.assertRaises(ValueError):
            resolve_clip("07-late-play", beats)


class RenderingTests(unittest.TestCase):
    def test_fit_size(self):
        self.assertEqual(fit_size("GULPER", 820, 110), 110)
        self.assertEqual(fit_size("GRAND THEFT AUTO-REPLY", 820, 110), 46)

    def test_fill_escapes_and_replaces_every_slot(self):
        out = fill("<t>{{TITLE}}</t><s>{{TITLE_SIZE}}</s>", TITLE="A & B", TITLE_SIZE=40)
        self.assertEqual(out, "<t>A &amp; B</t><s>40</s>")

    def test_card_args_add_silent_audio_and_fixed_format(self):
        a = card_args("card.png", 1.5, (1920, 1080), "seg.mkv")
        self.assertEqual(a[:6], ["-loop", "1", "-t", "1.500", "-i", "card.png"])
        self.assertIn("anullsrc=r=48000:cl=stereo", a)
        self.assertIn("scale=1920:1080,fps=60,format=yuv420p", a)
        self.assertIn("pcm_s16le", a)
        self.assertNotIn("aac", a)
        self.assertEqual(a[-1], "seg.mkv")

    def test_clip_args_trim_fade_and_normalize(self):
        a = clip_args("c.mp4", 1.0, 4.5, (1080, 1920), True, "seg.mkv")
        self.assertEqual(a[:6], ["-ss", "1.000", "-t", "4.500", "-i", "c.mp4"])
        af = a[a.index("-af") + 1]
        self.assertIn("afade=t=in:d=0.15", af)
        self.assertIn("afade=t=out:st=4.350:d=0.15", af)
        self.assertIn("loudnorm=I=-16:TP=-1.5:LRA=11", af)
        self.assertNotIn("anullsrc=r=48000:cl=stereo", a)
        self.assertIn("pcm_s16le", a)
        self.assertNotIn("aac", a)

    def test_clip_args_without_audio_use_silence(self):
        a = clip_args("c.mp4", 1.0, 4.5, (1920, 1080), False, "seg.mkv")
        self.assertIn("anullsrc=r=48000:cl=stereo", a)
        self.assertIn("1:a", a)

    def test_clip_args_without_normalize_skips_loudnorm(self):
        a = clip_args("c.mp4", 1.0, 4.5, (1080, 1920), True, "seg.mkv", normalize=False)
        af = a[a.index("-af") + 1]
        self.assertIn("afade=t=in:d=0.15", af)
        self.assertIn("afade=t=out:st=4.350:d=0.15", af)
        self.assertNotIn("loudnorm", af)

    def test_concat_list_quotes_paths(self):
        self.assertEqual(concat_list(["/a/b.mp4", "/it's.mp4"]),
                         "file '/a/b.mp4'\nfile '/it'\\''s.mp4'\n")


class SilenceDetectionTests(unittest.TestCase):
    def test_parse_max_volume_reads_db_value(self):
        stderr = "[Parsed_volumedetect_0 @ 0x600000cd0000] max_volume: -91.0 dB\n"
        self.assertEqual(parse_max_volume(stderr), -91.0)

    def test_parse_max_volume_returns_none_when_absent(self):
        self.assertIsNone(parse_max_volume("no volume info in this output\n"))

    @mock.patch("reel.subprocess.run")
    def test_is_near_silent_true_below_threshold(self, mock_run):
        mock_run.return_value = subprocess.CompletedProcess(
            args=[], returncode=0, stdout="", stderr="max_volume: -91.0 dB\n")
        self.assertTrue(is_near_silent("c.mp4", 1.0, 4.5))

    @mock.patch("reel.subprocess.run")
    def test_is_near_silent_true_when_no_volume_reported(self, mock_run):
        mock_run.return_value = subprocess.CompletedProcess(
            args=[], returncode=0, stdout="", stderr="")
        self.assertTrue(is_near_silent("c.mp4", 1.0, 4.5))

    @mock.patch("reel.subprocess.run")
    def test_is_near_silent_false_above_threshold(self, mock_run):
        mock_run.return_value = subprocess.CompletedProcess(
            args=[], returncode=0, stdout="", stderr="max_volume: -18.3 dB\n")
        self.assertFalse(is_near_silent("c.mp4", 1.0, 4.5))


class RunTests(unittest.TestCase):
    @mock.patch("reel.subprocess.run")
    def test_run_exits_with_labeled_last_stderr_line_on_failure(self, mock_run):
        mock_run.return_value = subprocess.CompletedProcess(
            args=[], returncode=234, stdout="",
            stderr="some warning\nInput contains (near) NaN/+-Inf\n")
        with self.assertRaises(SystemExit) as ctx:
            run("tire-stack clip", ["-i", "c.mp4", "out.mkv"])
        self.assertEqual(str(ctx.exception),
                         "reel: ffmpeg failed on tire-stack clip: Input contains (near) NaN/+-Inf")

    @mock.patch("reel.subprocess.run")
    def test_run_succeeds_silently_on_zero_exit(self, mock_run):
        mock_run.return_value = subprocess.CompletedProcess(args=[], returncode=0, stdout="", stderr="")
        run("ok", ["-i", "c.mp4", "out.mkv"])


if __name__ == "__main__":
    unittest.main()
