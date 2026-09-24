-- The first product migration proves that product schema ownership is separate
-- from Temporal's persistence and visibility schemas.
CREATE TABLE product_bootstrap (
    id integer PRIMARY KEY CHECK (id = 1),
    created_at timestamptz NOT NULL DEFAULT now()
);
INSERT INTO product_bootstrap (id) VALUES (1);
