import unittest

from cut_clips import (
    CTA,
    banner_svg,
    clip_args,
    clip_window,
    fmt_time,
    milestones,
    render_chapters,
    still_time,
    title_size,
    vertical_args,
    vertical_filter,
)


class ClipWindowTests(unittest.TestCase):
    def test_window_is_two_before_four_after(self):
        self.assertEqual(clip_window(10.0, 60.0), (8.0, 6.0))

    def test_window_clamps_to_start(self):
        self.assertEqual(clip_window(1.0, 60.0), (0.0, 5.0))

    def test_window_clamps_to_end(self):
        self.assertEqual(clip_window(58.0, 60.0), (56.0, 4.0))


class StillTimeTests(unittest.TestCase):
    def test_inside_range_is_unchanged(self):
        self.assertEqual(still_time(10.0, 60.0, 60), 10.0)

    def test_past_end_is_clamped_to_last_frame(self):
        self.assertAlmostEqual(still_time(60.0, 60.0, 60), 60.0 - 1.0 / 60)
        self.assertAlmostEqual(still_time(75.0, 60.0, 60), 60.0 - 1.0 / 60)

    def test_clamp_never_goes_negative_on_a_near_zero_duration(self):
        self.assertEqual(still_time(0.0, 0.0, 60), 0.0)


class FormatTests(unittest.TestCase):
    def test_fmt_time(self):
        self.assertEqual(fmt_time(0.0), "00:00.00")
        self.assertEqual(fmt_time(65.5), "01:05.50")

    def test_fmt_time_rounding_carries_to_minutes(self):
        self.assertEqual(fmt_time(119.996), "02:00.00")
        self.assertEqual(fmt_time(59.997), "01:00.00")


class MilestoneTests(unittest.TestCase):
    def test_first_score_and_power_of_ten_crossings(self):
        scores = [(1.0, 0), (2.0, 100), (3.0, 500), (4.0, 1000), (5.0, 1200), (6.0, 10500)]
        self.assertEqual(milestones(scores), [(2.0, 100), (4.0, 1000), (6.0, 10500)])

    def test_no_scores_gives_nothing(self):
        self.assertEqual(milestones([]), [])

    def test_float_scores_coerced_to_int(self):
        scores = [(1.0, 100.0), (2.0, 1000.0)]
        self.assertEqual(milestones(scores), [(1.0, 100), (2.0, 1000)])


class ChaptersTests(unittest.TestCase):
    def test_renders_table_and_clip_list(self):
        manifest = {"name": "Tire Stack", "fps": 60, "duration_s": 62.0, "frames": 3720}
        events = [
            {"frame": 1, "t": 0.017, "kind": "state", "data": {"name": "StudioLogo"}},
            {"frame": 72, "t": 1.2, "kind": "beat", "data": {"name": "01-studio-logo"}},
            {"frame": 400, "t": 6.667, "kind": "score", "data": {"value": 0}},
            {"frame": 520, "t": 8.667, "kind": "score", "data": {"value": 100}},
            {"frame": 3000, "t": 50.0, "kind": "pause", "data": {"value": 1}},
            {"frame": 3010, "t": 50.167, "kind": "SfxEvent", "data": {"debug": "Pause"}},
        ]
        out = render_chapters(manifest, events, ["clips/01-studio-logo.mp4"])
        self.assertIn("# Tire Stack — chapters", out)
        self.assertIn("| 00:00.02 | state | StudioLogo |", out)
        self.assertIn("| 00:01.20 | beat | 01-studio-logo |", out)
        self.assertIn("| 00:08.67 | score | 100 |", out)
        self.assertIn("| 00:50.00 | pause | on |", out)
        self.assertNotIn("SfxEvent", out)
        self.assertIn("- clips/01-studio-logo.mp4", out)
        self.assertIn("Tour length: 01:02.00 (3720 frames @ 60 fps)", out)


class VerticalTests(unittest.TestCase):
    def test_title_size_caps_short_titles_and_shrinks_long_ones(self):
        self.assertEqual(title_size("GULPER"), 96)
        self.assertEqual(title_size("GRAND THEFT AUTO-REPLY"), 51)

    def test_banner_svg_fills_and_escapes(self):
        tpl = "<t s='{{TITLE_SIZE}}'>{{TITLE}}</t><a>{{CTA_1}}</a><b>{{CTA_2}}</b>"
        out = banner_svg(tpl, "Dough & <Co>", CTA)
        self.assertIn("DOUGH &amp; &lt;CO&gt;", out)
        # title_size("DOUGH & <CO>") is 12 glyphs: int(900 / (0.8 * 12)) == 93,
        # below the 96 cap (verified against the two title_size tests above,
        # which pin width=900/cap=96/0.8-per-glyph; 96 is unreachable at this
        # length without breaking the 22-char-title == 51 case).
        self.assertIn("s='93'", out)
        self.assertIn("<a>Play the demo at</a><b>colecovisiongx.com</b>", out)
        self.assertNotIn("{{", out)

    def test_vertical_filter_blurs_background_and_centers_gameplay(self):
        f = vertical_filter()
        self.assertIn("crop=1080:1920", f)
        self.assertIn("gblur", f)
        self.assertIn("eq=brightness=-0.12", f)
        self.assertIn("[fg]scale=1080:-2", f)
        self.assertIn("overlay=(W-w)/2:(H-h)/2", f)
        self.assertTrue(f.endswith("[base][1:v]overlay=0:0[v]"))

    def test_vertical_args_map_optional_audio(self):
        a = vertical_args("in.mp4", "banner.png", "out.mp4")
        self.assertEqual(a[:4], ["-i", "in.mp4", "-i", "banner.png"])
        self.assertIn("0:a?", a)
        self.assertEqual(a[-1], "out.mp4")

    def test_clip_args_keep_audio(self):
        a = clip_args("tour.mp4", 8.0, 6.0, "c.mp4")
        self.assertNotIn("-an", a)
        self.assertIn("0:a?", a)
        self.assertIn("aac", a)
        self.assertEqual(a[:6], ["-ss", "8.000", "-i", "tour.mp4", "-t", "6.000"])


if __name__ == "__main__":
    unittest.main()
