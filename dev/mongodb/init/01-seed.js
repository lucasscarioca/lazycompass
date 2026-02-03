db = db.getSiblingDB("lazycompass");

db.users.drop();
db.orders.drop();
db.events.drop();

db.users.insertMany([
  {
    _id: 1,
    name: "Ada Lovelace",
    email: "ada@example.com",
    active: true,
    role: "admin",
    createdAt: new Date("2025-10-01T10:00:00Z"),
  },
  {
    _id: 2,
    name: "Grace Hopper",
    email: "grace@example.com",
    active: true,
    role: "engineer",
    createdAt: new Date("2025-10-05T09:30:00Z"),
  },
  {
    _id: 3,
    name: "Katherine Johnson",
    email: "katherine@example.com",
    active: false,
    role: "analyst",
    createdAt: new Date("2025-10-10T14:15:00Z"),
  },
]);

db.orders.insertMany([
  {
    _id: 1001,
    userId: 1,
    status: "paid",
    total: 125.5,
    items: [
      { sku: "keyboard", qty: 1, price: 85.0 },
      { sku: "mouse", qty: 1, price: 40.5 },
    ],
    createdAt: new Date("2025-10-20T12:00:00Z"),
  },
  {
    _id: 1002,
    userId: 2,
    status: "refunded",
    total: 60.0,
    items: [{ sku: "headset", qty: 1, price: 60.0 }],
    createdAt: new Date("2025-10-21T16:45:00Z"),
  },
  {
    _id: 1003,
    userId: 2,
    status: "paid",
    total: 220.0,
    items: [
      { sku: "monitor", qty: 2, price: 110.0 },
    ],
    createdAt: new Date("2025-10-22T08:20:00Z"),
  },
]);

db.events.insertMany([
  {
    _id: "evt_1",
    type: "login",
    userId: 1,
    at: new Date("2025-10-22T09:00:00Z"),
  },
  {
    _id: "evt_2",
    type: "query",
    userId: 2,
    at: new Date("2025-10-22T09:05:00Z"),
  },
  {
    _id: "evt_3",
    type: "logout",
    userId: 1,
    at: new Date("2025-10-22T09:10:00Z"),
  },
]);
