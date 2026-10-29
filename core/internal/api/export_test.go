package api

// BoundedRangeForTest exposes the range capping to the external test package.
//
// The capping is the whole reason playback works — upstream refuses an
// unbounded request — so it is worth testing directly rather than only through
// a relayed response.
func BoundedRangeForTest(clientRange string, size int64) string {
	return boundedRange(clientRange, size)
}

// NoFreshGrace makes every URL count as old for the rest of the test: a
// refusal then means stale at once.
func NoFreshGrace(t interface{ Cleanup(func()) }) {
	old := freshFor
	freshFor = 0
	t.Cleanup(func() { freshFor = old })
}
