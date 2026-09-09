import unittest

from cut_clips import clip_window, fmt_time, milestones, render_chapters


class ClipWindowTests(unittest.TestCase):
    def test_window_is_two_before_four_after(self):
        self.assertEqual(clip_window(10.0, 60.0), (8.0, 6.0))

    def test_window_clamps_to_start(self):
        self.assertEqual(clip_window(1.0, 60.0), (0.0, 5.0))

    def test_window_clamps_to_end(self):
        self.assertEqual(clip_window(58.0, 60.0), (56.0, 4.0))


class FormatTests(unittest.TestCase):
    def test_fmt_time(self):
        self.assertEqual(fmt_time(0.0), "00:00.00")
        self.assertEqual(fmt_time(65.5), "01:05.50")


class MilestoneTests(unittest.TestCase):
    def test_first_score_and_power_of_ten_crossings(self):
        scores = [(1.0, 0), (2.0, 100), (3.0, 500), (4.0, 1000), (5.0, 1200), (6.0, 10500)]
        self.assertEqual(milestones(scores), [(2.0, 100), (4.0, 1000), (6.0, 10500)])

    def test_no_scores_gives_nothing(self):
        self.assertEqual(milestones([]), [])


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


if __name__ == "__main__":
    unittest.main()
