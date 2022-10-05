import { defineStore } from "pinia";
import _ from "lodash";
import { watch } from "vue";

import storage from "local-storage-fallback";
import { ApiRequest } from "@/utils/pinia_api_tools";

import { ChangeSet, ChangeSetStatus } from "@/api/sdf/dal/change_set";
import { LabelList } from "@/api/sdf/dal/label_list";
import { addStoreHooks } from "@/utils/pinia_hooks_plugin";
import { changeSet$, eventChangeSetWritten$ } from "@/service/change_set";
import { useWorkspacesStore } from "./workspaces.store";
import { useRouterStore } from "./router.store";
import { useRealtimeStore } from "./realtime/realtime.store";

export type ChangeSetId = number;

export function useChangeSetsStore() {
  const workspacesStore = useWorkspacesStore();
  const workspaceId = workspacesStore.selectedWorkspaceId;

  return addStoreHooks(
    defineStore(`w${workspaceId || "NONE"}/change-sets`, {
      state: () => ({
        changeSetsById: {} as Record<ChangeSetId, ChangeSet>,
      }),
      getters: {
        allChangeSets: (state) => _.values(state.changeSetsById),
        openChangeSets(): ChangeSet[] {
          return _.filter(
            this.allChangeSets,
            (cs) => cs.status === ChangeSetStatus.Open,
          );
        },
        selectedChangeSetId(): ChangeSetId | null {
          // selecting HEAD = -1
          // TODO: this is all a bit confusing...
          const routerStore = useRouterStore();
          const urlSelectedChangeSetId = routerStore.urlSelectedChangeSetId;
          if (!urlSelectedChangeSetId) return -1;
          return this.selectedChangeSet?.id || null;
        },
        selectedChangeSet: (state) => {
          const routerStore = useRouterStore();
          const urlSelectedChangeSetId = routerStore.urlSelectedChangeSetId;
          return urlSelectedChangeSetId
            ? state.changeSetsById[urlSelectedChangeSetId as ChangeSetId]
            : null;
        },

        // expose here so other stores can get it without needing to call useWorkspaceStore directly
        selectedWorkspaceId: () => workspaceId,
      },
      actions: {
        async FETCH_CHANGE_SETS() {
          return new ApiRequest<{ list: LabelList<number> }>({
            method: "get",
            // TODO: probably want to fetch all change sets, not just open (or could have a filter)
            // this endpoint currently returns dropdown-y data, should just return the change set data itself
            url: "change_set/list_open_change_sets",
            headers: { WorkspaceId: workspaceId },
            onSuccess: (response) => {
              // this.changeSetsById = _.keyBy(response.changeSets, "id");

              // endpoint returns a dropdown list so we'll temporarily re-format into ChangeSet data
              const changeSetData = _.map(
                response.list,
                (ci) =>
                  ({
                    id: ci.value,
                    pk: ci.value,
                    name: ci.label,
                    // note: null,
                    status: ChangeSetStatus.Open,
                  } as ChangeSet),
              );

              this.changeSetsById = _.keyBy(changeSetData, "id");
            },
          });
        },
        async CREATE_CHANGE_SET(name: string) {
          return new ApiRequest<{ changeSet: ChangeSet }>({
            method: "post",
            url: "change_set/create_change_set",
            params: {
              changeSetName: name,
            },
            headers: { WorkspaceId: workspaceId },
            onSuccess: (response) => {
              this.changeSetsById[response.changeSet.id] = response.changeSet;
            },
          });
        },
        async APPLY_CHANGE_SET() {
          if (!this.selectedChangeSet) throw new Error("Select a change set");
          return new ApiRequest<{ changeSet: ChangeSet }>({
            method: "post",
            url: "change_set/apply_change_set",
            params: {
              changeSetPk: this.selectedChangeSet.pk,
            },
            onSuccess: (response) => {
              this.changeSetsById[response.changeSet.id] = response.changeSet;
              // could switch to head here, or could let the caller decide...
            },
          });
        },
        // TODO: async CANCEL_CHANGE_SET() {},

        // other related endpoints, not necessarily needed at the moment, but available
        // - change_set/get_change_set
        // - change_set/update_selected_change_set (was just fetching the change set info)

        getAutoSelectedChangeSetId() {
          console.log(this.openChangeSets);
          // returning `false` means we cannot auto select
          if (!this.openChangeSets.length) return false; // no open change sets
          if (this.openChangeSets.length === 1)
            return this.openChangeSets[0].id; // only 1 change set - will auto select it
          // TODO: add logic to for auto-selecting when multiple change sets open
          // - select one created by you
          // - track last selected in localstorage and select that one...
          const lastChangeSetIdRaw = storage.getItem(
            `SI:LAST_CHANGE_SET/${workspaceId}`,
          );
          if (!lastChangeSetIdRaw) return false;
          const lastChangeSetId = parseInt(lastChangeSetIdRaw);
          if (
            this.changeSetsById[lastChangeSetId]?.status ===
            ChangeSetStatus.Open
          ) {
            return lastChangeSetId;
          }
          return false;
        },
      },
      onActivated() {
        if (!workspaceId) return;
        console.log("ACTIVATE CHANGE SETS STORE", workspaceId);
        this.FETCH_CHANGE_SETS();
        const stopWatchSelectedChangeSet = watch(
          () => this.selectedChangeSet,
          () => {
            // pass along selected change set to rxjs
            changeSet$.next(this.selectedChangeSet);

            // store last used change set (per workspace) in localstorage
            if (this.selectedChangeSet && workspaceId) {
              storage.setItem(
                `SI:LAST_CHANGE_SET/${workspaceId}`,
                this.selectedChangeSet.id.toString(),
              );
            }
          },
          { immediate: true },
        );

        const realtimeStore = useRealtimeStore();
        // TODO: if selected change set gets cancelled/applied, need to show error if by other user, and switch to head...
        realtimeStore.subscribe(this.$id, `workspace/${workspaceId}`, [
          {
            eventType: "ChangeSetCreated",
            callback: this.FETCH_CHANGE_SETS,
          },
          {
            eventType: "ChangeSetCancelled",
            callback: this.FETCH_CHANGE_SETS,
          },
          {
            eventType: "ChangeSetApplied",
            callback: this.FETCH_CHANGE_SETS,
          },
          {
            eventType: "ChangeSetWritten",
            callback: () => {
              // this observable is just used as a signal...
              eventChangeSetWritten$.next(true);
            },
          },
        ]);

        return () => {
          console.log("DEACTIVATE CHANGE SETvS STORE", workspaceId);
          stopWatchSelectedChangeSet();
          realtimeStore.unsubscribe(this.$id);
        };
      },
    }),
  )();
}
