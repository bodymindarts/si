import * as PIXI from "pixi.js";
import * as OBJ from "../obj";

import { SchematicGroup, NodeGroup, ConnectionGroup } from "../group";
import { Renderer } from "../renderer";
import { Grid, BACKGROUND_GRID_NAME } from "../obj";
import { untilUnmounted } from "vuse-rx";
import { InteractionManager } from "../interaction";
import { SchematicKind } from "@/api/sdf/dal/schematic";
import {
  Schematic,
  variantById,
  inputSocketById,
} from "@/api/sdf/dal/schematic";

export type SceneGraphData = Schematic;

interface Point {
  x: number;
  y: number;
}

export class SceneManager {
  renderer: Renderer;
  scene: PIXI.Container;
  root: PIXI.Container;
  interactiveConnection?: OBJ.Connection | null;
  group: {
    nodes: PIXI.Container;
    connections: PIXI.Container;
  };
  zoomFactor?: number;

  constructor(renderer: Renderer) {
    this.renderer = renderer;
    this.scene = new PIXI.Container();
    this.scene.name = "scene";
    this.scene.interactive = true;
    this.scene.sortableChildren = true;

    this.scene.hitArea = new PIXI.Rectangle(
      0,
      0,
      renderer.width,
      renderer.height,
    );

    this.root = new PIXI.Container();
    this.root.name = "root";
    this.root.sortableChildren = true;
    this.root.zIndex = 2;
    this.scene.addChild(this.root);

    this.group = {
      connections: new ConnectionGroup("connections", 20),
      nodes: new NodeGroup("nodes", 30),
    };

    this.initializeSceneData();
    this.setBackgroundGrid(renderer.width, renderer.height);

    this.zoomFactor = 1;
  }

  subscribeToInteractionEvents(interactionManager: InteractionManager) {
    interactionManager.zoomFactor$.pipe(untilUnmounted).subscribe({
      next: (v) => this.updateZoomFactor(v),
    });
  }

  updateZoomFactor(zoomFactor: number | null) {
    if (zoomFactor) {
      this.zoomFactor = zoomFactor;
      const grid = this.root.getChildByName(BACKGROUND_GRID_NAME, true) as Grid;
      grid.updateZoomFactor(zoomFactor);
      grid.render(this.renderer);
    }
  }

  setBackgroundGrid(rendererWidth: number, rendererHeight: number): void {
    const grid = new Grid(rendererWidth, rendererHeight);
    grid.zIndex = 1;
    this.root.addChild(grid);
  }

  initializeSceneData(): void {
    this.clearSceneData();

    this.group = {
      connections: new ConnectionGroup("connections", 20),
      nodes: new NodeGroup("nodes", 30),
    };
    this.root.addChild(this.group.nodes);
    this.root.addChild(this.group.connections);
  }

  async loadSceneData(
    data: Schematic | null,
    schematicKind: SchematicKind,
    selectedDeploymentNodeId?: number,
  ): Promise<void> {
    this.initializeSceneData();

    if (data) {
      for (const n of data.nodes) {
        const variant = await variantById(n.schemaVariantId);

        const pos = n.positions.find(
          (pos) =>
            pos.schematicKind === schematicKind &&
            pos.deploymentNodeId === selectedDeploymentNodeId,
        );
        if (pos) {
          const node = new OBJ.Node(
            n,
            variant,
            {
              x: pos.x,
              y: pos.y,
            },
            schematicKind,
          );
          this.addNode(node);
        } else {
          // console.error("Node didn't have a position:", n);
        }
      }

      for (const connection of data.connections) {
        const sourceSocketId = `${connection.sourceNodeId}.${connection.sourceSocketId}`;
        const sourceSocket = this.scene.getChildByName(
          sourceSocketId,
          true,
        ) as OBJ.Socket;

        // Sometimes the connection isn't valid for display, like when switching panels while rendering
        // And the "include" connections also won't be found as they don't get rendered, we could use some metadata,
        // but there isn't much to gain from it
        if (!sourceSocket) continue;

        const destinationSocketId = `${connection.destinationNodeId}.${connection.destinationSocketId}`;
        const destinationSocket = this.scene.getChildByName(
          destinationSocketId,
          true,
        );

        const socket = await inputSocketById(sourceSocket.id);
        this.createConnection(
          sourceSocket.getGlobalPosition(),
          destinationSocket.getGlobalPosition(),
          sourceSocket.name,
          destinationSocket.name,
          socket.provider.color,
        );
      }
    }

    this.renderer.renderStage();
  }

  clearSceneData(): void {
    for (let i = 0; i < this.root.children.length; i++) {
      const group = this.root.children[i] as SchematicGroup | Grid;
      if (group instanceof NodeGroup || group instanceof ConnectionGroup) {
        this.root.removeChild(group);
      }
    }
  }

  getSocketPosition(socketId: string): PIXI.Point {
    const socket = this.scene.getChildByName(socketId, true);
    const position = socket.getGlobalPosition();
    return position;
  }

  getGeo(name: string): PIXI.DisplayObject {
    const geo = this.renderer.stage.getChildByName(name, true);
    return geo;
  }

  addNode(n: OBJ.Node): void {
    this.group.nodes.addChild(n);
  }

  removeNode(node: OBJ.Node): void {
    node.destroy();

    const nodeGroup = this.scene.getChildByName(this.group.nodes.name, true);
    this.renderer.renderGroup(nodeGroup);
  }

  translateNode(node: OBJ.Node, position: Point): void {
    node.x = position.x;
    node.y = position.y;
    node.updateTransform();
  }

  createConnection(
    p1: Point,
    p2: Point,
    sourceSocketId: string,
    destinationSocketId: string,
    color: number,
    _interactive?: boolean,
  ): OBJ.Connection | null {
    const connection = new OBJ.Connection(
      p1,
      p2,
      sourceSocketId,
      destinationSocketId,
      color,
      _interactive,
    );
    let isConnectionUnique = true;
    for (const c of this.group.connections.children) {
      const conn = c as OBJ.Connection;
      if (conn.name === connection.name) {
        isConnectionUnique = false;
      }
    }

    for (const node of this.group.nodes.children) {
      const sockets = node.getChildByName("Sockets");
      if (sockets) {
        const source = sockets.getChildByName(sourceSocketId);
        if (source) source.setConnected();

        const destination = sockets.getChildByName(destinationSocketId);
        if (destination) destination.setConnected();
      }
    }

    if (isConnectionUnique) {
      this.addConnection(connection);
      this.refreshConnections(); // inefficient, should be for the connections on a node.
      // this.renderConnection(connection); // causes an orphan edge to renders.
      return connection;
    } else {
      return null;
    }
  }

  addConnection(c: OBJ.Connection): void {
    this.group.connections.addChild(c);
  }

  removeConnection(name: string): void {
    const c = this.scene.getChildByName(name, true) as OBJ.Connection;
    this.group.connections.removeChild(c);
  }

  refreshConnections(): void {
    for (const c of this.group.connections.children) {
      const connection = c as OBJ.Connection;
      if (connection && connection.type != OBJ.ConnectionType.interactive) {
        this.refreshConnectionPosition(connection.name);
      }
    }
  }

  refreshConnectionPosition(name: string): void {
    const c = this.scene.getChildByName(name, true) as OBJ.Connection;
    const sp = this.getSocketPosition(c.sourceSocketId);
    const dp = this.getSocketPosition(c.destinationSocketId);

    //  target.worldTransform.tx) * (1 / zoomFactor)
    if (this.zoomFactor != null) {
      const offset = {
        x: this.root.x,
        y: this.root.y,
      };

      const p1 = {
        x: (sp.x - offset.x) * (1 / this.zoomFactor),
        y: (sp.y - offset.y) * (1 / this.zoomFactor),
      };

      const p2 = {
        x: (dp.x - offset.x) * (1 / this.zoomFactor),
        y: (dp.y - offset.y) * (1 / this.zoomFactor),
      };
      c.update(p1, p2);
    }
  }

  updateConnectionInteractive(name: string, p: Point): void {
    const c = this.scene.getChildByName(name, true) as OBJ.Connection;

    if (c && this.interactiveConnection) {
      const p1 = {
        x: this.interactiveConnection.x,
        y: this.interactiveConnection.y,
      };
      const p2 = {
        x: p.x,
        y: p.y,
      };
      c.update(p1, p2);
    }
  }

  getConnections(): void {
    const connections = this.group.connections.children;
    console.log(connections);
  }

  renderConnection(c: OBJ.Connection): void {
    c.render(this.renderer);
  }
}
