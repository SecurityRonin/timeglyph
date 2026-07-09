"""Verify the Timesketch analyzer's logic against the real timeglyph wheel, with
the Timesketch interface mocked (Timesketch itself need not be installed)."""
import importlib.util
import os
import sys
import types


def _load_analyzer():
    # Mock the timesketch interface + manager so the analyzer imports standalone.
    analyzers = types.ModuleType("timesketch.lib.analyzers")

    class BaseAnalyzer:
        def event_stream(self, **_):
            return getattr(self, "_events", [])

    analyzers.interface = types.SimpleNamespace(BaseAnalyzer=BaseAnalyzer)
    analyzers.manager = types.SimpleNamespace(
        AnalysisManager=types.SimpleNamespace(register_analyzer=lambda cls: None)
    )
    sys.modules["timesketch"] = types.ModuleType("timesketch")
    sys.modules["timesketch.lib"] = types.ModuleType("timesketch.lib")
    sys.modules["timesketch.lib.analyzers"] = analyzers

    path = os.path.join(os.path.dirname(__file__), "timeglyph_analyzer.py")
    spec = importlib.util.spec_from_file_location("tg_analyzer", path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


class FakeEvent:
    def __init__(self, value):
        self.source = {"raw_value": value}
        self.attributes = {}
        self.tags = []

    def add_attributes(self, a):
        self.attributes.update(a)

    def add_tags(self, t):
        self.tags += t

    def commit(self):
        pass


def test_analyzer_annotates_a_unix_value():
    mod = _load_analyzer()
    analyzer = mod.TimeglyphAnalyzer.__new__(mod.TimeglyphAnalyzer)
    event = FakeEvent("1577836800")
    analyzer._events = [event]
    result = analyzer.run()
    assert event.attributes["timeglyph_format"] == "unix"
    assert "2020-01-01" in event.attributes["timeglyph_time"]
    assert event.attributes["timeglyph_citation"]
    assert "timeglyph" in event.tags
    assert "1 event" in result


def test_analyzer_ignores_non_timestamp():
    mod = _load_analyzer()
    analyzer = mod.TimeglyphAnalyzer.__new__(mod.TimeglyphAnalyzer)
    event = FakeEvent("hello")
    analyzer._events = [event]
    analyzer.run()
    assert not event.tags
