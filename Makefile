VERSION := $(shell sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
MAJOR := $(word 1,$(subst ., ,$(VERSION)))
MINOR := $(word 2,$(subst ., ,$(VERSION)))
PATCH := $(word 3,$(subst ., ,$(VERSION)))

.PHONY: version check-version ver-release ver-feature ver-bug

version:
	@echo $(VERSION)

check-version:
	@if [ -z "$$CI_TAG" ]; then echo "CI_TAG not set"; exit 1; fi
	@if [ "v$(VERSION)" != "$$CI_TAG" ]; then \
		echo "error: Cargo.toml version v$(VERSION) does not match tag $$CI_TAG"; exit 1; \
	fi
	@echo "Version check passed: v$(VERSION)"

ver-bug:
	@$(MAKE) _bump NEW_VERSION=$(MAJOR).$(MINOR).$(shell echo $$(($(PATCH)+1)))
ver-feature:
	@$(MAKE) _bump NEW_VERSION=$(MAJOR).$(shell echo $$(($(MINOR)+1))).0
ver-release:
	@$(MAKE) _bump NEW_VERSION=$(shell echo $$(($(MAJOR)+1))).0.0

_bump:
	@if [ -z "$(NEW_VERSION)" ]; then echo "error: NEW_VERSION not set"; exit 1; fi
	@echo "$(VERSION) -> $(NEW_VERSION)"
	@perl -pi -e 's/^version = "$(VERSION)"/version = "$(NEW_VERSION)"/' Cargo.toml
	@cargo generate-lockfile --quiet
	@git add Cargo.toml Cargo.lock
	@git commit -m "v$(NEW_VERSION)"
	@git tag "v$(NEW_VERSION)"
	@echo ""
	@echo "Ready. Push with:"
	@echo "  git push origin main v$(NEW_VERSION)"
