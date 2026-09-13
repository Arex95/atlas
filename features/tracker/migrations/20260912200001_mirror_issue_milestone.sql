-- Asking how far along a roadmap is means grouping issues the way the
-- team already groups them, and that is a milestone. The mirror has to
-- carry it or every such question goes to the network.
--
-- Empty string rather than NULL for "no milestone", so a row written
-- before this column existed and a row for an issue genuinely outside
-- every milestone read the same. The mapping turns both back into
-- `None`; there is no third state worth distinguishing, and a nullable
-- column would invent one.
ALTER TABLE mirror_issues ADD COLUMN milestone TEXT NOT NULL DEFAULT '';
