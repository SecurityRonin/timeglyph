"""Timesketch analyzer: re-interpret numeric timestamp-like fields with timeglyph.

Install the wheel (`pip install timeglyph`) alongside Timesketch, drop this file
in the analyzers directory, and register it. For every event carrying a numeric
field that decodes as a high-confidence timestamp, it annotates the event with
timeglyph's top reading (format + rendered time + citation) and tags it — surfacing
mis-parsed or unrecognised time fields in a super-timeline.
"""
from timesketch.lib.analyzers import interface, manager

import timeglyph


class TimeglyphAnalyzer(interface.BaseAnalyzer):
    """Annotate events whose value field decodes as a plausible timestamp."""

    NAME = "timeglyph"
    DISPLAY_NAME = "timeglyph timestamp identifier"
    DESCRIPTION = (
        "Flags events whose numeric field decodes as a high-confidence "
        "timestamp, annotating the top ranked, cited reading."
    )

    # Field to interpret; override per-sketch as needed.
    VALUE_FIELD = "raw_value"

    def run(self):
        query = f'_exists_:{self.VALUE_FIELD}'
        events = self.event_stream(query_string=query, return_fields=[self.VALUE_FIELD])
        matched = 0
        for event in events:
            value = str(event.source.get(self.VALUE_FIELD, "")).strip()
            if not value:
                continue
            readings = timeglyph.identify(value)
            if not readings or readings[0]["score"] < 0.9:
                continue
            top = readings[0]
            event.add_attributes(
                {
                    "timeglyph_format": top["format_id"],
                    "timeglyph_time": top.get("rendered", ""),
                    "timeglyph_citation": top.get("citation", ""),
                }
            )
            event.add_tags(["timeglyph"])
            event.commit()
            matched += 1
        return f"timeglyph identified {matched} event(s)"


manager.AnalysisManager.register_analyzer(TimeglyphAnalyzer)
