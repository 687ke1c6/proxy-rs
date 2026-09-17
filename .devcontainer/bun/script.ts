import express from "express";

const app = express();

app.get("/", (_req, res) => {
  res.status(200).send("200 Hello, World!");
});

app.listen(3000, () => {
  console.log("Server is running on port 3000");
});